use crate::VaultData;
use anyhow::{Context, Result, ensure};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"SNRVAULT";
const FORMAT_VERSION: u16 = 1;
const HEADER_LEN: usize = 8 + 2 + 4 + 4 + 4 + 16 + 24;
const MAX_VAULT_BYTES: u64 = 256 * 1024 * 1024;
const AUTH_FAILURE: &str = "Senha incorreta ou cofre danificado; nenhum dado foi alterado";

#[derive(Clone, Copy)]
struct KdfParams {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

impl KdfParams {
    fn validate(self) -> Result<()> {
        ensure!(
            (8 * 1024..=256 * 1024).contains(&self.memory_kib)
                && (1..=10).contains(&self.iterations)
                && (1..=4).contains(&self.parallelism),
            "Parâmetros do cofre inválidos ou não suportados"
        );
        Ok(())
    }
}

/// Exclusive owner of an unlocked vault. No password is retained. `key` is
/// zeroized on drop, and the stable sidecar lock is released by closing its file.
pub struct Vault {
    path: PathBuf,
    _lock: File,
    key: Zeroizing<[u8; 32]>,
    salt: [u8; 16],
    params: KdfParams,
    // Encrypted bytes only. Compare before save to detect non-cooperating writers.
    last_disk_bytes: Vec<u8>,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault").finish_non_exhaustive()
    }
}

impl Vault {
    pub fn create(path: &Path, password: &str) -> Result<(Self, VaultData)> {
        Self::create_with_params(path, password, KdfParams::default())
    }

    fn create_with_params(
        path: &Path,
        password: &str,
        params: KdfParams,
    ) -> Result<(Self, VaultData)> {
        validate_password(password)?;
        let path = normalize_path(path, true)?;
        let lock = acquire_lock(&path)?;
        ensure!(
            !path.try_exists()?,
            "Já existe um cofre nesse local; abra-o em vez de sobrescrever"
        );
        ensure!(
            !backup_path(&path).try_exists()?,
            "Existe um backup de cofre nesse local; recupere-o antes de criar outro"
        );
        let salt = random_array::<16>()?;
        let key = derive_key(password, &salt, params)?;
        let data = VaultData::default();
        let encoded = encode(&data, &key, &salt, params)?;
        atomic_write(&path, &encoded, false)?;
        sync_parent(&path)?;
        Ok((
            Self {
                path,
                _lock: lock,
                key,
                salt,
                params,
                last_disk_bytes: encoded,
            },
            data,
        ))
    }

    pub fn open(path: &Path, password: &str) -> Result<(Self, VaultData)> {
        validate_password(password)?;
        let path = normalize_path(path, false)?;
        let lock = acquire_lock(&path)?;
        let encoded = read_bounded(&path).context("Não foi possível ler o cofre")?;
        let (data, key, salt, params) = decode(&encoded, password)?;
        Ok((
            Self {
                path,
                _lock: lock,
                key,
                salt,
                params,
                last_disk_bytes: encoded,
            },
            data,
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> PathBuf {
        backup_path(&self.path)
    }

    /// A successful save writes ciphertext only: temp file -> fsync -> atomic
    /// replacement. The previous committed encrypted file becomes `.bak` first.
    /// Interrupted temp files cannot be mistaken for the live vault.
    pub fn save(&mut self, data: &VaultData) -> Result<()> {
        data.validate()?;
        let encoded = encode(data, &self.key, &self.salt, self.params)?;
        self.ensure_unchanged()?;
        atomic_write(&backup_path(&self.path), &self.last_disk_bytes, true)
            .context("Não foi possível salvar o backup; o cofre original foi preservado")?;
        sync_parent(&self.path)?;
        atomic_write(&self.path, &encoded, true)
            .context("Não foi possível substituir o cofre; o backup está preservado")?;
        self.last_disk_bytes = encoded;
        // Set the baseline before directory sync: if the OS reports a durability
        // error after rename, a retry must not see our own write as external.
        sync_parent(&self.path)
    }

    /// Re-encrypt both active data and backup under the new password. Old backup
    /// history is intentionally replaced so the old password does not open it.
    pub fn change_password(&mut self, password: &str, data: &VaultData) -> Result<()> {
        validate_password(password)?;
        data.validate()?;
        let salt = random_array::<16>()?;
        let params = KdfParams::default();
        let key = derive_key(password, &salt, params)?;
        let encoded = encode(data, &key, &salt, params)?;
        self.ensure_unchanged()?;
        atomic_write(&backup_path(&self.path), &encoded, true)?;
        sync_parent(&self.path)?;
        atomic_write(&self.path, &encoded, true)?;
        self.key = key;
        self.salt = salt;
        self.params = params;
        self.last_disk_bytes = encoded;
        sync_parent(&self.path)
    }

    /// Explicit recovery, never an automatic fallback on a wrong password. A
    /// damaged primary is retained as encrypted `.damaged`; the verified backup
    /// is copied atomically into place while holding the primary's lock.
    pub fn recover_from_backup(path: &Path, password: &str) -> Result<(Self, VaultData)> {
        validate_password(password)?;
        let path = normalize_path(path, false)?;
        let lock = acquire_lock(&path)?;
        let encoded =
            read_bounded(&backup_path(&path)).context("Não foi possível ler o backup do cofre")?;
        let (data, key, salt, params) = decode(&encoded, password)?;
        if path.try_exists()? {
            let damaged = read_bounded(&path)?;
            let damaged_path = append_suffix(&path, ".damaged");
            // Do not overwrite a previously retained damaged file.
            if !damaged_path.try_exists()? {
                atomic_write(&damaged_path, &damaged, false)?;
            } else {
                let unique = append_suffix(&path, &format!(".damaged-{}", uuid::Uuid::new_v4()));
                atomic_write(&unique, &damaged, false)?;
            }
        }
        atomic_write(&path, &encoded, true)?;
        sync_parent(&path)?;
        Ok((
            Self {
                path,
                _lock: lock,
                key,
                salt,
                params,
                last_disk_bytes: encoded,
            },
            data,
        ))
    }

    fn ensure_unchanged(&self) -> Result<()> {
        let current = read_bounded(&self.path)
            .context("O cofre foi removido ou não pode ser lido; gravação cancelada")?;
        ensure!(
            current == self.last_disk_bytes,
            "O arquivo do cofre mudou externamente; gravação cancelada para preservar ambos os estados"
        );
        Ok(())
    }
}

fn validate_password(password: &str) -> Result<()> {
    ensure!(!password.is_empty(), "Informe uma senha para o cofre");
    ensure!(
        password.len() <= 1024 * 1024,
        "Senha excede o tamanho suportado"
    );
    Ok(())
}

fn random_array<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("O sistema não forneceu aleatoriedade segura"))?;
    Ok(bytes)
}

fn derive_key(password: &str, salt: &[u8; 16], params: KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    params.validate()?;
    let config = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(32),
    )
    .map_err(|_| anyhow::anyhow!("Parâmetros Argon2 inválidos"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, config);
    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key[..])
        .map_err(|_| anyhow::anyhow!("Não foi possível derivar a chave do cofre"))?;
    Ok(key)
}

fn make_header(params: KdfParams, salt: &[u8; 16], nonce: &[u8; 24]) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    header.extend_from_slice(&params.memory_kib.to_le_bytes());
    header.extend_from_slice(&params.iterations.to_le_bytes());
    header.extend_from_slice(&params.parallelism.to_le_bytes());
    header.extend_from_slice(salt);
    header.extend_from_slice(nonce);
    header
}

fn encode(data: &VaultData, key: &[u8; 32], salt: &[u8; 16], params: KdfParams) -> Result<Vec<u8>> {
    data.validate()?;
    let plaintext =
        Zeroizing::new(serde_json::to_vec(data).context("Falha ao serializar o cofre")?);
    ensure!(
        plaintext.len() as u64 + HEADER_LEN as u64 + 16 <= MAX_VAULT_BYTES,
        "O cofre excede o limite de 256 MiB"
    );
    let nonce_bytes = random_array::<24>()?;
    let mut header = make_header(params, salt, &nonce_bytes);
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("Chave de cofre inválida"))?;
    let nonce = XNonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &plaintext,
                aad: &header,
            },
        )
        .map_err(|_| anyhow::anyhow!("Falha ao criptografar o cofre"))?;
    header.extend_from_slice(&ciphertext);
    Ok(header)
}

type DecodedVault = (VaultData, Zeroizing<[u8; 32]>, [u8; 16], KdfParams);

fn decode(encoded: &[u8], password: &str) -> Result<DecodedVault> {
    ensure!(
        encoded.len() >= HEADER_LEN + 16 && encoded.len() as u64 <= MAX_VAULT_BYTES,
        "Cofre truncado ou com tamanho não suportado"
    );
    ensure!(
        &encoded[..8] == MAGIC,
        "O arquivo não é um cofre Standard Notes Rune"
    );
    ensure!(
        u16::from_le_bytes(encoded[8..10].try_into()?) == FORMAT_VERSION,
        "Versão de cofre não suportada; arquivo preservado"
    );
    let params = KdfParams {
        memory_kib: u32::from_le_bytes(encoded[10..14].try_into()?),
        iterations: u32::from_le_bytes(encoded[14..18].try_into()?),
        parallelism: u32::from_le_bytes(encoded[18..22].try_into()?),
    };
    params.validate()?;
    let salt: [u8; 16] = encoded[22..38].try_into()?;
    let nonce_bytes: [u8; 24] = encoded[38..62].try_into()?;
    let key = derive_key(password, &salt, params)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key[..]).map_err(|_| anyhow::anyhow!(AUTH_FAILURE))?;
    let nonce = XNonce::from(nonce_bytes);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &encoded[HEADER_LEN..],
                    aad: &encoded[..HEADER_LEN],
                },
            )
            .map_err(|_| anyhow::anyhow!(AUTH_FAILURE))?,
    );
    let data: VaultData = serde_json::from_slice(&plaintext)
        .context("Conteúdo interno do cofre inválido; arquivo preservado")?;
    data.validate()?;
    Ok((data, key, salt, params))
}

fn normalize_path(path: &Path, create_parent: bool) -> Result<PathBuf> {
    let filename = path
        .file_name()
        .context("Informe um caminho de arquivo para o cofre")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if create_parent {
        fs::create_dir_all(parent).context("Não foi possível criar a pasta do cofre")?;
    }
    let parent = parent
        .canonicalize()
        .context("A pasta do cofre não existe ou não está acessível")?;
    let path = parent.join(filename);
    reject_symlink(&path)?;
    Ok(path)
}

fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            !metadata.file_type().is_symlink(),
            "O arquivo do cofre não pode ser um link simbólico"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Não foi possível verificar o arquivo do cofre"),
    }
    Ok(())
}

fn acquire_lock(path: &Path) -> Result<File> {
    let lock_path = append_suffix(path, ".lock");
    reject_symlink(&lock_path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(lock_path)
        .context("Não foi possível abrir o bloqueio do cofre")?;
    FileExt::try_lock_exclusive(&file)
        .context("Este cofre já está aberto em outro processo; feche a outra janela")?;
    Ok(file)
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn backup_path(path: &Path) -> PathBuf {
    append_suffix(path, ".bak")
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    reject_symlink(path)?;
    let file = File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "O caminho do cofre não é um arquivo normal"
    );
    let mut bytes = Vec::new();
    file.take(MAX_VAULT_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_VAULT_BYTES,
        "O cofre excede o limite de 256 MiB"
    );
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    reject_symlink(path)?;
    let parent = path.parent().context("Caminho do cofre inválido")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .context("Não foi possível criar arquivo temporário ao lado do cofre")?;
    temporary
        .write_all(bytes)
        .context("Não foi possível gravar o cofre; verifique o espaço em disco")?;
    temporary
        .as_file()
        .sync_all()
        .context("Não foi possível sincronizar o cofre com o disco")?;
    if replace {
        temporary
            .persist(path)
            .map_err(|error| error.error)
            .context("Falha na substituição atômica do cofre")?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|error| error.error)
            .context("O arquivo já existe ou não pôde ser criado")?;
    }
    Ok(())
}

fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path.parent().context("Pasta do cofre inválida")?)?
            .sync_all()
            .context("O sistema não confirmou a gravação durável da pasta do cofre")?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Item;

    fn test_params() -> KdfParams {
        KdfParams {
            memory_kib: 8 * 1024,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, Vault, VaultData) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cofre-ação.snr");
        let (vault, mut data) =
            Vault::create_with_params(&path, "senha de teste", test_params()).unwrap();
        data.items.push(Item::new_note(
            "Plano João",
            "Açúcar 📝\nSegunda linha\0final",
        ));
        (directory, path, vault, data)
    }

    #[test]
    fn encrypted_roundtrip_preserves_unicode_and_never_writes_plaintext() {
        let (_directory, path, mut vault, data) = fixture();
        vault.save(&data).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows("Açúcar".len())
                .any(|window| window == "Açúcar".as_bytes())
        );
        assert!(
            !bytes
                .windows("Plano João".len())
                .any(|window| window == "Plano João".as_bytes())
        );
        drop(vault);
        let (_vault, loaded) = Vault::open(&path, "senha de teste").unwrap();
        assert_eq!(data, loaded);
    }

    #[test]
    fn wrong_password_and_tampering_do_not_change_files() {
        let (_directory, path, mut vault, data) = fixture();
        vault.save(&data).unwrap();
        drop(vault);
        let before = fs::read(&path).unwrap();
        assert!(Vault::open(&path, "senha errada").is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        for offset in [22, 38, HEADER_LEN, before.len() - 1] {
            let mut changed = before.clone();
            changed[offset] ^= 0x80;
            fs::write(&path, &changed).unwrap();
            assert!(Vault::open(&path, "senha de teste").is_err());
            assert_eq!(fs::read(&path).unwrap(), changed);
        }
    }

    #[test]
    fn exclusive_lock_is_released_after_drop_and_error() {
        let (_directory, path, vault, _data) = fixture();
        assert!(Vault::open(&path, "senha de teste").is_err());
        assert!(Vault::create(&path, "outra senha").is_err());
        drop(vault);
        assert!(Vault::open(&path, "incorreta").is_err());
        assert!(Vault::open(&path, "senha de teste").is_ok());
    }

    #[test]
    fn external_change_is_detected_without_overwriting_either_file() {
        let (_directory, path, mut vault, data) = fixture();
        vault.save(&data).unwrap();
        let backup_before = fs::read(vault.backup_path()).unwrap();
        fs::write(&path, b"externally changed encrypted file").unwrap();
        assert!(vault.save(&data).is_err());
        assert_eq!(
            fs::read(&path).unwrap(),
            b"externally changed encrypted file"
        );
        assert_eq!(fs::read(vault.backup_path()).unwrap(), backup_before);
    }

    #[test]
    fn invalid_state_and_backup_failure_leave_original_vault_intact() {
        let (_directory, path, mut vault, mut data) = fixture();
        let before = fs::read(&path).unwrap();
        data.items.push(data.items[0].clone());
        assert!(vault.save(&data).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        data.items.pop();
        fs::create_dir(vault.backup_path()).unwrap();
        assert!(vault.save(&data).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn backup_recovery_is_explicit_and_retains_damaged_primary() {
        let (_directory, path, mut vault, mut data) = fixture();
        vault.save(&data).unwrap();
        let first = data.clone();
        data.items[0].set_text("edição posterior").unwrap();
        vault.save(&data).unwrap();
        drop(vault);
        fs::write(&path, b"damaged").unwrap();
        assert!(Vault::open(&path, "senha de teste").is_err());
        let (_vault, recovered) = Vault::recover_from_backup(&path, "senha de teste").unwrap();
        assert_eq!(recovered, first);
        assert_eq!(
            fs::read(append_suffix(&path, ".damaged")).unwrap(),
            b"damaged"
        );
    }

    #[test]
    fn same_plaintext_gets_fresh_nonce_on_every_save() {
        let (_directory, path, mut vault, data) = fixture();
        vault.save(&data).unwrap();
        let previous = fs::read(&path).unwrap();
        vault.save(&data).unwrap();
        let current = fs::read(&path).unwrap();
        assert_ne!(&previous[38..62], &current[38..62]);
        assert_ne!(previous, current);
    }

    #[test]
    fn attacker_controlled_kdf_parameters_are_bounded() {
        let (_directory, path, vault, _data) = fixture();
        drop(vault);
        let mut bytes = fs::read(&path).unwrap();
        bytes[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(
            Vault::open(&path, "senha de teste")
                .unwrap_err()
                .to_string()
                .contains("Parâmetros")
        );
    }

    #[test]
    fn changing_password_rekeys_primary_and_backup() {
        let (_directory, path, mut vault, data) = fixture();
        vault.save(&data).unwrap();
        vault.change_password("nova senha de teste", &data).unwrap();
        let backup = fs::read(vault.backup_path()).unwrap();
        assert!(decode(&backup, "senha de teste").is_err());
        assert!(decode(&backup, "nova senha de teste").is_ok());
        drop(vault);
        assert!(Vault::open(&path, "senha de teste").is_err());
        let (_vault, loaded) = Vault::open(&path, "nova senha de teste").unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn creation_never_overwrites_existing_file_or_orphaned_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.snr");
        fs::write(&path, b"existing").unwrap();
        assert!(Vault::create(&path, "password").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing");
        fs::remove_file(&path).unwrap();
        fs::write(backup_path(&path), b"recovery candidate").unwrap();
        assert!(Vault::create(&path, "password").is_err());
        assert!(!path.exists());
    }

    #[test]
    fn another_process_cannot_open_the_same_vault() {
        let (_directory, path, _vault, _data) = fixture();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "vault::tests::lock_process_probe", "--ignored"])
            .env("SN_RUNE_LOCK_PROBE", &path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Child process did not observe the exclusive lock"
        );
    }

    #[test]
    #[ignore = "Executed by another_process_cannot_open_the_same_vault"]
    fn lock_process_probe() {
        let path = std::env::var_os("SN_RUNE_LOCK_PROBE").expect("test probe path");
        let error = Vault::open(Path::new(&path), "senha de teste").unwrap_err();
        assert!(error.to_string().contains("outro processo"));
    }
}
