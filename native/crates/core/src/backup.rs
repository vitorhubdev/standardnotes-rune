use crate::{Item, VaultData};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};
use uuid::Uuid;

pub const MAX_IMPORT_BYTES: u64 = 128 * 1024 * 1024;

/// Use this API instead of the item-only convenience functions when preserving
/// an entire .sn envelope (including fields supplied by other clients).
#[derive(Clone, Default)]
pub struct BackupDocument {
    pub items: Vec<Item>,
    pub metadata: BTreeMap<String, Value>,
}

impl std::fmt::Debug for BackupDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackupDocument")
            .field("item_count", &self.items.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added: usize,
    pub skipped: usize,
    /// Conflicting UUIDs were assigned new IDs; existing items were not changed.
    pub conflicts: usize,
}

pub fn import_document(text: &str) -> Result<BackupDocument> {
    ensure!(
        text.len() as u64 <= MAX_IMPORT_BYTES,
        "O backup excede o limite de 128 MiB"
    );
    let input: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .context("Arquivo .sn inválido; era esperado JSON UTF-8")?;
    let mut envelope = input
        .as_object()
        .context("O backup .sn deve ser um objeto JSON")?
        .clone();
    let raw_items = envelope
        .remove("items")
        .context("O backup .sn não possui o campo items")?;
    let raw_items = raw_items
        .as_array()
        .context("O campo items deve ser uma lista")?;
    // Reject the whole import before returning anything; never silently skip
    // encrypted/unknown items or import only the readable subset.
    if envelope.get("keyParams").is_some_and(|v| !v.is_null())
        || envelope.get("auth_params").is_some_and(|v| !v.is_null())
        || raw_items
            .iter()
            .any(|item| item.get("content").is_some_and(Value::is_string))
    {
        bail!(
            "Backup Standard Notes criptografado: use a importação com senha; nenhum item foi importado"
        );
    }
    let mut items = Vec::with_capacity(raw_items.len());
    let mut seen = BTreeSet::new();
    for raw in raw_items {
        let item: Item =
            serde_json::from_value(raw.clone()).context("Item inválido no backup .sn")?;
        item.validate()?;
        ensure!(
            seen.insert(item.uuid.clone()),
            "O backup contém UUIDs repetidos; importação cancelada"
        );
        items.push(item);
    }
    Ok(BackupDocument {
        items,
        metadata: envelope.into_iter().collect(),
    })
}

pub fn import_backup(text: &str) -> Result<Vec<Item>> {
    Ok(import_document(text)?.items)
}

/// Export content and metadata without passwords. As in the upstream decrypted
/// backup, cryptographic key records are excluded. Use the encrypted local vault
/// or the protocol crate's encrypted backup for a complete key-bearing backup.
pub fn export_document(document: &BackupDocument) -> Result<String> {
    let mut envelope: serde_json::Map<String, Value> =
        document.metadata.clone().into_iter().collect();
    ensure!(
        !envelope.contains_key("items"),
        "Metadado de backup colide com items"
    );
    // A plaintext export must not masquerade as an encrypted .sn document.
    ensure!(
        !envelope.get("keyParams").is_some_and(|v| !v.is_null())
            && !envelope.get("auth_params").is_some_and(|v| !v.is_null()),
        "Metadados de backup criptografado não pertencem a uma exportação sem senha"
    );
    let mut seen = BTreeSet::new();
    let mut items = Vec::with_capacity(document.items.len());
    for item in &document.items {
        if matches!(
            item.content_type.as_str(),
            "SN|ItemsKey"
                | "SN|RootKey|NoSync"
                | "SN|EncryptedStorage"
                | "SN|KeySystemRootKey"
                | "SN|KeySystemItemsKey"
        ) {
            continue;
        }
        item.validate()?;
        ensure!(
            seen.insert(&item.uuid),
            "Não é possível exportar UUIDs repetidos"
        );
        // Preserve all fields of exportable items. The decrypted export never
        // includes account sessions (VaultData.account) or key-bearing records.
        items.push(serde_json::to_value(item).context("Falha ao serializar item")?);
    }
    envelope
        .entry("version")
        .or_insert_with(|| Value::String("004".to_owned()));
    envelope.insert("items".to_owned(), Value::Array(items));
    serde_json::to_string_pretty(&envelope).context("Falha ao serializar backup")
}

pub fn export_backup(items: &[Item]) -> Result<String> {
    export_document(&BackupDocument {
        items: items.to_vec(),
        metadata: BTreeMap::new(),
    })
}

pub fn import_text(title: &str, text: &str) -> Item {
    Item::new_note(title, text.strip_prefix('\u{feff}').unwrap_or(text))
}

pub fn import_file(path: &Path) -> Result<Vec<Item>> {
    let mut file = File::open(path).context("Não foi possível abrir o arquivo para importação")?;
    ensure!(
        file.metadata()?.is_file(),
        "O caminho de importação não é um arquivo"
    );
    let mut contents = String::new();
    file.by_ref()
        .take(MAX_IMPORT_BYTES + 1)
        .read_to_string(&mut contents)
        .context("O arquivo deve usar texto UTF-8")?;
    ensure!(
        contents.len() as u64 <= MAX_IMPORT_BYTES,
        "O arquivo excede o limite de 128 MiB"
    );
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "sn" | "json" => import_backup(&contents),
        "txt" | "md" | "markdown" => {
            let title = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Nota importada");
            Ok(vec![import_text(title, &contents)])
        }
        _ => bail!("Formato não suportado; selecione .sn, .json, .txt ou .md"),
    }
}

/// Merge an entire source document. Conflicting envelope fields keep the first
/// value at the top level and retain the incoming envelope in an explicit source
/// history field, so importing several backups never discards metadata.
pub fn merge_document(data: &mut VaultData, document: BackupDocument) -> Result<ImportReport> {
    ensure!(
        !document.metadata.contains_key("items"),
        "Metadado de backup colide com items"
    );
    let report = merge_import(data, document.items)?;
    let has_conflict = document.metadata.iter().any(|(key, value)| {
        data.backup_metadata
            .get(key)
            .is_some_and(|existing| existing != value)
    });
    if has_conflict {
        let snapshot =
            serde_json::to_value(&document.metadata).context("Metadados de backup inválidos")?;
        let base = "_standardnotes_rune_source_envelopes";
        let mut key = base.to_owned();
        let mut suffix = 1usize;
        while data
            .backup_metadata
            .get(&key)
            .is_some_and(|value| !value.is_array())
        {
            key = format!("{base}_{suffix}");
            suffix += 1;
        }
        let history = data.backup_metadata.entry(key).or_insert_with(|| json!([]));
        let history = history.as_array_mut().expect("checked array");
        if !history.contains(&snapshot) {
            history.push(snapshot);
        }
    }
    for (key, value) in document.metadata {
        data.backup_metadata.entry(key).or_insert(value);
    }
    Ok(report)
}

/// Stage all transformations before touching `data`. A collision becomes an
/// explicit duplicate, and references within the imported set follow the new
/// IDs. Existing notes and their relationships are never overwritten.
pub fn merge_import(data: &mut VaultData, incoming: Vec<Item>) -> Result<ImportReport> {
    data.validate()?;
    let existing: BTreeMap<&str, &Item> = data
        .items
        .iter()
        .map(|item| (item.uuid.as_str(), item))
        .collect();
    let mut incoming_ids = BTreeSet::new();
    for item in &incoming {
        item.validate()?;
        ensure!(
            incoming_ids.insert(item.uuid.as_str()),
            "A importação contém UUIDs repetidos"
        );
    }
    let mut used: BTreeSet<String> = existing
        .keys()
        .map(|id| (*id).to_owned())
        .chain(incoming_ids.iter().map(|id| (*id).to_owned()))
        .collect();
    let mut remap = BTreeMap::<String, String>::new();
    // References can cause a previously equal tag/note to need duplication too.
    // The mapping grows monotonically and therefore converges after <= n passes.
    loop {
        let mut changed = false;
        for item in &incoming {
            if remap.contains_key(&item.uuid) {
                continue;
            }
            if let Some(current) = existing.get(item.uuid.as_str()) {
                let mut mapped = item.clone();
                remap_references(&mut mapped, &remap);
                if !current.equivalent_for_import(&mapped) {
                    let id = loop {
                        let candidate = Uuid::new_v4().to_string();
                        if used.insert(candidate.clone()) {
                            break candidate;
                        }
                    };
                    remap.insert(item.uuid.clone(), id);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut additions = Vec::new();
    let mut report = ImportReport::default();
    for mut item in incoming {
        let source_id = item.uuid.clone();
        remap_references(&mut item, &remap);
        if let Some(new_id) = remap.get(&source_id) {
            item.uuid = new_id.clone();
            let original_duplicate = item.extra.get("duplicate_of").cloned();
            let original_timestamp = item.extra.get("updated_at_timestamp").cloned();
            item.extra
                .insert("duplicate_of".to_owned(), Value::String(source_id));
            // A new item has never been synchronized. Keep its original server
            // timestamps in a named metadata object for provenance.
            let mut provenance_key = "rune_import_origin".to_owned();
            let mut provenance_suffix = 1usize;
            while item.extra.contains_key(&provenance_key) {
                provenance_key = format!("rune_import_origin_{provenance_suffix}");
                provenance_suffix += 1;
            }
            item.extra.insert(
                provenance_key,
                json!({
                    "created_at": item.created_at,
                    "updated_at": item.updated_at,
                    "server_updated_at": item.server_updated_at,
                    "updated_at_timestamp": original_timestamp,
                    "duplicate_of": original_duplicate,
                }),
            );
            item.updated_at = "1970-01-01T00:00:00.000Z".to_owned();
            item.extra
                .insert("updated_at_timestamp".to_owned(), json!(0));
            item.server_updated_at = None;
            report.conflicts += 1;
        } else if existing.contains_key(item.uuid.as_str()) {
            report.skipped += 1;
            continue;
        }
        item.dirty = true;
        item.validate()?;
        additions.push(item);
        report.added += 1;
    }
    data.items.extend(additions);
    Ok(report)
}

fn remap_references(item: &mut Item, remap: &BTreeMap<String, String>) {
    if let Some(references) = item
        .content
        .get_mut("references")
        .and_then(Value::as_array_mut)
    {
        for reference in references {
            if let Some(old) = reference.get("uuid").and_then(Value::as_str)
                && let Some(new) = remap.get(old)
            {
                reference["uuid"] = Value::String(new.clone());
            }
        }
    }
}
