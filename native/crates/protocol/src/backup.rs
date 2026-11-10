use crate::crypto::{KeyParams, RootKey, derive_root, random_hex};
use crate::wire::{KeyRing, WireItem};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sn_rune_core::{BackupDocument, Item};
use std::collections::HashSet;

const MAX_BACKUP_BYTES: usize = 128 * 1024 * 1024;
const MAX_BACKUP_ITEMS: usize = 250_000;

/// Import protocol 004 backups, including offline backups containing a plaintext
/// items key. Decryption is all-or-nothing. Unsupported signatures, missing vault
/// keys and old protocols return an error before any caller data is changed.
pub fn import_encrypted_backup(json: &str, password: &str) -> Result<Vec<Item>> {
    Ok(import_encrypted_document(json, password)?.items)
}

/// Like `import_encrypted_backup`, preserving unknown top-level backup fields.
/// Encryption key parameters are consumed, not carried into a plaintext export.
pub fn import_encrypted_document(json: &str, password: &str) -> Result<BackupDocument> {
    ensure!(
        json.len() <= MAX_BACKUP_BYTES,
        "Backup excede o limite de 128 MiB."
    );
    let backup: Value = serde_json::from_str(json.trim_start_matches('\u{feff}'))
        .context("Arquivo de backup JSON inválido.")?;
    let array = backup
        .get("items")
        .and_then(Value::as_array)
        .context("Backup sem lista de itens.")?;
    ensure!(
        array.len() <= MAX_BACKUP_ITEMS,
        "Backup excede 250.000 itens."
    );
    let wire: Vec<WireItem> = serde_json::from_value(Value::Array(array.clone()))
        .context("Estrutura de backup inválida.")?;
    let mut unique = HashSet::new();
    for item in &wire {
        ensure!(
            unique.insert(&item.uuid),
            "Backup contém identificadores duplicados; nenhum dado foi importado."
        );
    }
    let root = if let Some(params) = backup
        .get("keyParams")
        .or_else(|| backup.get("auth_params"))
        .filter(|p| !p.is_null())
    {
        let params: KeyParams = serde_json::from_value(params.clone())
            .context("Parâmetros de backup incompletos ou formato legado.")?;
        derive_root(password, &params)?
    } else {
        // The master key is unused when every items key is already plaintext.
        // An encrypted root-based item will fail authentication, never be skipped.
        RootKey {
            master_key: random_hex(32),
            server_password: String::new(),
            params: KeyParams::new("offline-backup"),
        }
    };
    let mut ring = KeyRing::new(&root, &[])?;
    let items = ring.decrypt_all(&wire, false)?;
    let mut metadata = backup
        .as_object()
        .context("Envelope de backup inválido.")?
        .clone();
    for field in ["items", "keyParams", "auth_params"] {
        metadata.remove(field);
    }
    Ok(BackupDocument {
        items,
        metadata: metadata.into_iter().collect(),
    })
}

/// Export a standard .sn protocol 004 backup protected by the supplied password.
/// This uses the Standard Notes wire format, distinct from the local Rune vault.
/// The account login, sync tokens and local application preferences are excluded.
pub fn export_encrypted_backup(items: &[Item], password: &str) -> Result<String> {
    export_encrypted_document(
        &BackupDocument {
            items: items.to_vec(),
            metadata: Default::default(),
        },
        password,
    )
}

/// Export with the unknown envelope metadata retained from an earlier import.
pub fn export_encrypted_document(document: &BackupDocument, password: &str) -> Result<String> {
    ensure!(
        !password.is_empty(),
        "Defina uma senha para proteger o backup."
    );
    let params = KeyParams::new(format!("rune-backup-{}", uuid::Uuid::new_v4()));
    let root = derive_root(password, &params)?;
    let mut source: Vec<_> = document
        .items
        .iter()
        .filter(|i| {
            i.content_type != "SN|RootKey|NoSync" && i.content_type != "SN|EncryptedStorage"
        })
        .cloned()
        .collect();
    let mut ring = KeyRing::new(&root, &source)?;
    if ring.default_key_id(None).is_none() {
        source.push(ring.add_new_items_key()?);
    }
    let encrypted = source
        .iter()
        .map(|item| ring.encrypt(item))
        .collect::<Result<Vec<_>>>()?;
    let mut envelope: serde_json::Map<String, Value> =
        document.metadata.clone().into_iter().collect();
    envelope.remove("auth_params");
    envelope.insert("version".into(), json!("004"));
    envelope.insert("keyParams".into(), serde_json::to_value(params)?);
    envelope.insert("items".into(), serde_json::to_value(encrypted)?);
    let output = serde_json::to_string_pretty(&envelope)?;
    ensure!(
        output.len() <= MAX_BACKUP_BYTES,
        "Backup excede o limite de 128 MiB."
    );
    Ok(output)
}
