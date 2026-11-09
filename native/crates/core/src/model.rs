use anyhow::{Context, Result, bail, ensure};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
use zeroize::Zeroize;

pub const APP_DOMAIN: &str = "org.standardnotes.sn";
const EPOCH: &str = "1970-01-01T00:00:00.000Z";

/// All unfamiliar top-level fields and all content fields survive serialization.
/// `updated_at` belongs to the server. Local edits update client_updated_at.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub uuid: String,
    pub content_type: String,
    #[serde(default)]
    pub content: Value,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_updated_at: Option<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("uuid", &self.uuid)
            .field("content_type", &self.content_type)
            .field("deleted", &self.deleted)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

impl Item {
    pub fn new_note(title: &str, text: &str) -> Self {
        let mut item = Self::new("Note", title);
        item.content["text"] = Value::String(text.to_owned());
        item.content["noteType"] = Value::String("plain-text".to_owned());
        item
    }

    pub fn new_tag(title: &str) -> Self {
        let mut item = Self::new("Tag", title);
        item.content["expanded"] = Value::Bool(true);
        item
    }

    fn new(content_type: &str, title: &str) -> Self {
        let now = Utc::now();
        let date = now.to_rfc3339_opts(SecondsFormat::Millis, true);
        Self {
            uuid: Uuid::new_v4().to_string(),
            content_type: content_type.to_owned(),
            content: json!({
                "title": title,
                "references": [],
                "appData": {APP_DOMAIN: {"client_updated_at": date}},
            }),
            created_at: date,
            updated_at: EPOCH.to_owned(),
            deleted: false,
            dirty: true,
            server_updated_at: None,
            extra: BTreeMap::from([
                // Match upstream PayloadTimestampDefaults. The sync protocol
                // supplies its own wire timestamp units; imported values remain
                // unchanged, including legacy clients using different units.
                ("created_at_timestamp".to_owned(), json!(0)),
                ("updated_at_timestamp".to_owned(), json!(0)),
            ]),
        }
    }

    pub fn title(&self) -> &str {
        self.content
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    pub fn text(&self) -> &str {
        self.content
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    pub fn modified_at(&self) -> &str {
        self.app_data()
            .and_then(|v| v.get("client_updated_at"))
            .and_then(Value::as_str)
            .unwrap_or(&self.updated_at)
    }

    pub fn is_note(&self) -> bool {
        self.content_type == "Note"
    }

    pub fn is_tag(&self) -> bool {
        // Older/custom clients may emit the namespaced form. New tags use the
        // canonical wire content type `Tag`, matching Standard Notes.
        self.content_type == "Tag" || self.content_type == "SN|Tag"
    }

    /// Plain-text compatibility check. Use `is_editable_text_source` for the
    /// source editor, which also supports upstream Markdown and code documents.
    pub fn is_editable_plain_text(&self) -> bool {
        self.is_editable_text_source()
            && self
                .content
                .get("noteType")
                .and_then(Value::as_str)
                .is_none_or(|kind| kind.is_empty() || kind == "plain-text")
            && self
                .content
                .get("editorIdentifier")
                .and_then(Value::as_str)
                .is_none_or(|id| id.is_empty() || id == "com.standardnotes.plain-text")
    }

    /// Edits a textual source without converting its format or editor metadata.
    /// The allowlist comes from upstream NoteType.ts, NativeFeatureIdentifier.ts
    /// and DeprecatedFeatures.ts. Rich HTML, Lexical, spreadsheets, tasks, and
    /// unfamiliar editors remain opaque/read-only.
    pub fn is_editable_text_source(&self) -> bool {
        if !self.is_note() || self.deleted || self.flag("locked") || self.flag("protected") {
            return false;
        }
        if !self.content.is_object()
            || self
                .content
                .get("text")
                .is_some_and(|value| !value.is_string())
        {
            return false;
        }
        if let Some(kind) = self.content.get("noteType")
            && !kind.is_null()
            && !matches!(kind.as_str(), Some("" | "plain-text" | "markdown" | "code"))
        {
            return false;
        }
        match self.content.get("editorIdentifier") {
            None | Some(Value::Null) => true,
            Some(Value::String(id)) => matches!(
                id.as_str(),
                "" | "com.standardnotes.plain-text"
                    | "org.standardnotes.code-editor"
                    | "org.standardnotes.advanced-markdown-editor"
                    | "org.standardnotes.markdown-visual-editor"
                    | "org.standardnotes.simple-markdown-editor"
                    | "org.standardnotes.fancy-markdown-editor"
                    | "org.standardnotes.minimal-markdown-editor"
            ),
            Some(_) => false,
        }
    }

    pub fn set_title(&mut self, title: &str) -> Result<()> {
        ensure!(
            !self.flag("locked") && !self.flag("protected"),
            "O item está protegido ou bloqueado para edição"
        );
        if self.title() == title {
            return Ok(());
        }
        self.check_mutable_content()?;
        self.content["title"] = Value::String(title.to_owned());
        self.touch()
    }

    pub fn set_text(&mut self, text: &str) -> Result<()> {
        ensure!(
            self.is_editable_text_source(),
            "Este editor não pode ser alterado como fonte textual; o conteúdo foi preservado"
        );
        if self.text() == text {
            return Ok(());
        }
        self.check_mutable_content()?;
        self.content["text"] = Value::String(text.to_owned());
        self.touch()
    }

    pub fn flag(&self, key: &str) -> bool {
        let value = match key {
            "pinned" | "archived" | "locked" => self.app_data().and_then(|v| v.get(key)),
            _ => self.content.get(key),
        };
        value.and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn set_flag(&mut self, key: &str, value: bool) -> Result<()> {
        ensure!(
            matches!(
                key,
                "pinned" | "archived" | "trashed" | "locked" | "protected" | "starred"
            ),
            "Atributo de nota desconhecido"
        );
        self.check_mutable_content()?;
        if self.flag(key) == value {
            return Ok(());
        }
        match key {
            "pinned" | "archived" | "locked" => {
                self.app_data_mut()?
                    .insert(key.to_owned(), Value::Bool(value));
            }
            _ => {
                self.content[key] = Value::Bool(value);
            }
        }
        self.touch()
    }

    /// Mark a direct content mutation while preserving server conflict timestamps.
    pub fn touch(&mut self) -> Result<()> {
        let date = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        self.app_data_mut()?
            .insert("client_updated_at".to_owned(), Value::String(date));
        self.dirty = true;
        self.extra.remove("signatureData");
        self.extra.remove("last_edited_by_uuid");
        if let Some(index) = self.extra.get("dirtyIndex").and_then(Value::as_u64) {
            self.extra
                .insert("dirtyIndex".to_owned(), json!(index.saturating_add(1)));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        Uuid::parse_str(&self.uuid).context("O item contém um UUID inválido")?;
        ensure!(
            !self.content_type.trim().is_empty(),
            "O item não possui content_type"
        );
        ensure!(
            self.content.is_object() || (self.deleted && self.content.is_null()),
            "Conteúdo de item inválido ou ainda criptografado"
        );
        for name in [
            "uuid",
            "content_type",
            "content",
            "created_at",
            "updated_at",
            "deleted",
            "dirty",
            "server_updated_at",
        ] {
            ensure!(
                !self.extra.contains_key(name),
                "Metadado de item colide com um campo reservado"
            );
        }
        Ok(())
    }

    fn app_data(&self) -> Option<&Map<String, Value>> {
        self.content.get("appData")?.get(APP_DOMAIN)?.as_object()
    }

    fn check_mutable_content(&self) -> Result<()> {
        ensure!(
            self.content.is_object(),
            "O conteúdo não é um objeto editável"
        );
        if let Some(app) = self.content.get("appData") {
            ensure!(
                app.is_object(),
                "appData inválido: conteúdo original preservado"
            );
            if let Some(domain) = app.get(APP_DOMAIN) {
                ensure!(
                    domain.is_object(),
                    "Domínio appData inválido: conteúdo original preservado"
                );
            }
        }
        Ok(())
    }

    fn app_data_mut(&mut self) -> Result<&mut Map<String, Value>> {
        self.check_mutable_content()?;
        let content = self.content.as_object_mut().expect("checked object");
        let app = content.entry("appData").or_insert_with(|| json!({}));
        let app = app.as_object_mut().expect("checked appData");
        Ok(app
            .entry(APP_DOMAIN)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("checked domain"))
    }

    pub(crate) fn equivalent_for_import(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.content_type == other.content_type
            && self.content == other.content
            && self.created_at == other.created_at
            && self.updated_at == other.updated_at
            && self.deleted == other.deleted
            && self.extra == other.extra
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VaultData {
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub settings: Map<String, Value>,
    #[serde(default)]
    pub account: Option<Value>,
    /// Envelope metadata from .sn exports, excluding the items array.
    #[serde(default)]
    pub backup_metadata: BTreeMap<String, Value>,
}

impl std::fmt::Debug for VaultData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultData")
            .field("item_count", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl VaultData {
    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for item in &self.items {
            item.validate()?;
            ensure!(
                ids.insert(item.uuid.as_str()),
                "O cofre contém UUIDs repetidos"
            );
        }
        Ok(())
    }

    /// Best-effort clearing of owned strings when locking. This does not erase
    /// copies owned by a UI toolkit or OS; callers must clear those separately.
    pub fn clear_sensitive(&mut self) {
        for item in &mut self.items {
            clear_value(&mut item.content);
            for value in item.extra.values_mut() {
                clear_value(value);
            }
        }
        self.items.clear();
        for value in self.settings.values_mut() {
            clear_value(value);
        }
        self.settings.clear();
        if let Some(account) = &mut self.account {
            clear_value(account);
        }
        self.account = None;
        for value in self.backup_metadata.values_mut() {
            clear_value(value);
        }
        self.backup_metadata.clear();
    }
}

fn clear_value(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(values) => values.iter_mut().for_each(clear_value),
        Value::Object(values) => values.values_mut().for_each(clear_value),
        _ => {}
    }
    *value = Value::Null;
}

pub fn tag_names(data: &VaultData, note_id: &str) -> Vec<String> {
    let note = data.items.iter().find(|item| item.uuid == note_id);
    let mut names = BTreeSet::new();
    for tag in data
        .items
        .iter()
        .filter(|item| item.is_tag() && !item.deleted)
    {
        let tag_has_note = has_reference(tag, note_id);
        let note_has_tag = note.is_some_and(|note| has_reference(note, &tag.uuid));
        if tag_has_note || note_has_tag {
            names.insert(tag.title().to_owned());
        }
    }
    names.into_iter().collect()
}

fn has_reference(item: &Item, target: &str) -> bool {
    item.content
        .get("references")
        .and_then(Value::as_array)
        .is_some_and(|refs| {
            refs.iter()
                .any(|reference| reference.get("uuid").and_then(Value::as_str) == Some(target))
        })
}

/// Transactional in-memory update. Tag -> Note links follow upstream TagMutator;
/// non-tag references and unfamiliar fields remain intact.
pub fn set_note_tags(data: &mut VaultData, note_id: &str, names: &[String]) -> Result<()> {
    ensure!(
        data.items
            .iter()
            .any(|item| item.uuid == note_id && item.is_note() && !item.deleted),
        "Nota não encontrada"
    );
    let desired: BTreeSet<String> = names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    let mut staged = data.items.clone();
    let mut selected = BTreeMap::<String, String>::new();
    for item in &staged {
        if item.is_tag() && !item.deleted && desired.contains(item.title()) {
            selected
                .entry(item.title().to_owned())
                .or_insert_with(|| item.uuid.clone());
        }
    }
    for title in desired
        .iter()
        .filter(|name| !selected.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>()
    {
        let tag = Item::new_tag(&title);
        selected.insert(title, tag.uuid.clone());
        staged.push(tag);
    }
    let selected_ids: BTreeSet<String> = selected.into_values().collect();
    for item in &mut staged {
        if !item.is_tag() || item.deleted {
            continue;
        }
        item.check_mutable_content()?;
        let original = item
            .content
            .get("references")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let mut refs = original
            .as_array()
            .context("Referências da tag inválidas: nenhuma alteração aplicada")?
            .clone();
        let wanted = selected_ids.contains(&item.uuid);
        refs.retain(|reference| {
            reference.get("uuid").and_then(Value::as_str) != Some(note_id) || wanted
        });
        if wanted
            && !refs
                .iter()
                .any(|reference| reference.get("uuid").and_then(Value::as_str) == Some(note_id))
        {
            refs.push(json!({"uuid": note_id, "content_type": "Note"}));
        }
        if original != Value::Array(refs.clone()) {
            item.content["references"] = Value::Array(refs);
            item.touch()?;
        }
    }
    // Preserve unrelated note references, and remove obsolete legacy note->tag links.
    if let Some(note) = staged.iter_mut().find(|item| item.uuid == note_id)
        && let Some(references) = note.content.get("references")
    {
        let original = references
            .as_array()
            .context("Referências da nota inválidas: nenhuma alteração aplicada")?;
        let filtered: Vec<Value> = original
            .iter()
            .filter(|reference| {
                let kind = reference.get("content_type").and_then(Value::as_str);
                !matches!(kind, Some("Tag" | "SN|Tag"))
                    || reference
                        .get("uuid")
                        .and_then(Value::as_str)
                        .is_some_and(|id| selected_ids.contains(id))
            })
            .cloned()
            .collect();
        if &filtered != original {
            note.check_mutable_content()?;
            note.content["references"] = Value::Array(filtered);
            note.touch()?;
        }
    }
    if staged.iter().any(|item| item.validate().is_err()) {
        bail!("Itens inválidos; nenhuma alteração aplicada");
    }
    data.items = staged;
    Ok(())
}
