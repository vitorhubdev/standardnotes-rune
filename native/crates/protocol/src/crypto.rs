use anyhow::{Context, Result, bail, ensure};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use zeroize::{Zeroize, Zeroizing};

/// Key derivation metadata. Unknown fields are retained for backup round trips.
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyParams {
    pub identifier: String,
    pub pw_nonce: String,
    pub version: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl KeyParams {
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            pw_nonce: random_hex(32),
            version: "004".into(),
            extra: BTreeMap::from([
                ("origination".into(), json!("registration")),
                (
                    "created".into(),
                    json!(chrono::Utc::now().timestamp_millis().to_string()),
                ),
            ]),
        }
    }
}

/// Account keys are deliberately not Debug. The account vault encrypts serialized
/// keys; callers must never write this structure to an unencrypted preference file.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct RootKey {
    pub(crate) master_key: String,
    pub(crate) server_password: String,
    pub(crate) params: KeyParams,
}

impl Drop for RootKey {
    fn drop(&mut self) {
        self.master_key.zeroize();
        self.server_password.zeroize();
    }
}

pub(crate) fn derive_root(password: &str, params: &KeyParams) -> Result<RootKey> {
    ensure!(
        params.version == "004",
        "Protocolo {} não suportado; use uma exportação descriptografada do aplicativo original.",
        params.version
    );
    ensure!(
        !params.identifier.is_empty() && !params.pw_nonce.is_empty(),
        "Parâmetros de chave incompletos."
    );
    ensure!(
        params.identifier.len() <= 4096 && params.pw_nonce.len() <= 4096,
        "Parâmetros de chave excessivos."
    );
    let salt_hash = Sha256::digest(format!("{}:{}", params.identifier, params.pw_nonce));
    let mut derived = Zeroizing::new([0u8; 64]);
    // libsodium crypto_pwhash_ALG_DEFAULT = Argon2id13; memory is KiB here.
    let config = Params::new(65_536, 5, 1, Some(64))
        .map_err(|_| anyhow::anyhow!("Parâmetros Argon2 inválidos."))?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, config)
        .hash_password_into(password.as_bytes(), &salt_hash[..16], derived.as_mut())
        .map_err(|_| anyhow::anyhow!("Não foi possível derivar a chave da conta."))?;
    Ok(RootKey {
        master_key: hex::encode(&derived[..32]),
        server_password: hex::encode(&derived[32..]),
        params: params.clone(),
    })
}

pub(crate) fn random_hex(bytes: usize) -> String {
    let mut data = Zeroizing::new(vec![0u8; bytes]);
    OsRng.fill_bytes(&mut data);
    hex::encode(data.as_slice())
}

/// JavaScript's sortedCopy sorts only top-level keys before JSON/base64 encoding.
/// Do this explicitly: other workspace crates enable serde_json preserve_order.
pub(crate) fn encode_json(value: &Value) -> Result<String> {
    let object = value
        .as_object()
        .context("Dados autenticados devem ser um objeto.")?;
    let sorted: BTreeMap<_, _> = object.iter().collect();
    Ok(STANDARD.encode(serde_json::to_vec(&sorted)?))
}

pub(crate) fn key_bytes(raw: &str) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = Zeroizing::new(hex::decode(raw).context("Chave hexadecimal inválida.")?);
    ensure!(bytes.len() == 32, "Chave criptográfica deve ter 32 bytes.");
    Ok(bytes)
}

pub(crate) fn encrypt_string(plaintext: &str, key: &str, aad: &Value) -> Result<String> {
    let raw_key = key_bytes(key)?;
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let aad = encode_json(aad)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&raw_key)
        .map_err(|_| anyhow::anyhow!("Chave inválida."))?;
    let encrypted = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_bytes(),
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("Falha ao criptografar item."))?;
    Ok(format!(
        "004:{}:{}:{}:e30=",
        hex::encode(nonce),
        STANDARD.encode(encrypted),
        aad
    ))
}

pub(crate) fn decrypt_string(
    encrypted: &str,
    key: &str,
    uuid: &str,
    key_system_identifier: Option<&str>,
    shared_vault_uuid: Option<&str>,
) -> Result<Zeroizing<String>> {
    let parts: Vec<_> = encrypted.split(':').collect();
    ensure!(
        (4..=5).contains(&parts.len()),
        "Payload criptografado malformado."
    );
    ensure!(
        parts[0] == "004",
        "Payload legado não suportado; nenhuma alteração foi aplicada."
    );
    let nonce = hex::decode(parts[1]).context("Nonce inválido.")?;
    ensure!(nonce.len() == 24, "Nonce inválido.");
    let raw_key = key_bytes(key)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&raw_key)
        .map_err(|_| anyhow::anyhow!("Chave inválida."))?;
    let aad_raw = STANDARD
        .decode(parts[3])
        .context("Autenticação do item inválida.")?;
    ensure!(
        aad_raw.len() <= 64 * 1024,
        "Autenticação do item excessiva."
    );
    let aad: Value = serde_json::from_slice(&aad_raw).context("Autenticação do item inválida.")?;
    let authenticated_uuid = aad
        .get("u")
        .and_then(Value::as_str)
        .context("Identificador autenticado ausente.")?;
    ensure!(
        authenticated_uuid == uuid || (authenticated_uuid == uuid.to_uppercase()),
        "Identificador do item não corresponde ao conteúdo autenticado."
    );
    ensure!(
        aad.get("v").and_then(Value::as_str) == Some("004"),
        "Versão autenticada inválida."
    );
    ensure!(
        aad.get("ksi").and_then(Value::as_str) == key_system_identifier,
        "Identificador de cofre não corresponde ao conteúdo autenticado."
    );
    ensure!(
        aad.get("svu").and_then(Value::as_str) == shared_vault_uuid,
        "Identificador compartilhado não corresponde ao conteúdo autenticado."
    );
    // Signed/shared payloads must not be treated as trusted unsigned private data.
    if shared_vault_uuid.is_some() {
        bail!("Cofres compartilhados ainda exigem o aplicativo original.");
    }
    if let Some(extra) = parts.get(4) {
        let extra: Value = serde_json::from_slice(
            &STANDARD
                .decode(extra)
                .context("Dados adicionais inválidos.")?,
        )?;
        ensure!(
            extra.get("signingData").is_none(),
            "Payload assinado exige verificação ainda não implementada; o original foi preservado."
        );
    }
    let ciphertext = STANDARD
        .decode(parts[2])
        .context("Conteúdo criptografado inválido.")?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: parts[3].as_bytes(),
                },
            )
            .map_err(|_| {
                anyhow::anyhow!(
                    "Senha/chave incorreta ou conteúdo alterado; nenhum dado foi importado."
                )
            })?,
    );
    Ok(Zeroizing::new(
        String::from_utf8(plaintext.to_vec()).context("Conteúdo descriptografado não é UTF-8.")?,
    ))
}

pub(crate) fn aad_for(uuid: &str, key_params: Option<&Value>, ksi: Option<&str>) -> Value {
    let mut aad = json!({"u":uuid,"v":"004"});
    if let Some(params) = key_params {
        aad["kp"] = params.clone();
    }
    if let Some(identifier) = ksi {
        aad["ksi"] = json!(identifier);
    }
    aad
}
