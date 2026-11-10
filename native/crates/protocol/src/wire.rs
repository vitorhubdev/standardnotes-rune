use crate::crypto::{RootKey, aad_for, decrypt_string, encrypt_string, key_bytes, random_hex};
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sn_rune_core::Item;
use std::collections::{BTreeMap, HashMap};
use zeroize::{Zeroize, Zeroizing};

pub(crate) const ITEMS_KEY: &str = "SN|ItemsKey";
const VAULT_ROOT: &str = "SN|KeySystemRootKey";
const VAULT_ITEMS_KEY: &str = "SN|KeySystemItemsKey";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct WireItem {
    pub uuid: String,
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enc_item_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items_key_id: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl WireItem {
    pub fn from_item(item: &Item) -> Self {
        let mut extra = item.extra.clone();
        let items_key_id = extra
            .remove("items_key_id")
            .and_then(|v| v.as_str().map(str::to_owned));
        for name in [
            "enc_item_key",
            "auth_hash",
            "dirty",
            "server_updated_at",
            "errorDecrypting",
            "signatureData",
        ] {
            extra.remove(name);
        }
        Self {
            uuid: item.uuid.clone(),
            content_type: item.content_type.clone(),
            content: item.content.clone(),
            enc_item_key: None,
            items_key_id,
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
            deleted: item.deleted,
            extra,
        }
    }

    pub fn into_item(mut self, remote: bool) -> Result<Item> {
        uuid::Uuid::parse_str(&self.uuid).context("Identificador de item inválido.")?;
        ensure!(!self.content_type.is_empty(), "Tipo de item ausente.");
        ensure!(
            self.deleted || self.content.is_object(),
            "Conteúdo do item não é um objeto descriptografado."
        );
        self.created_at = timestamp(&self.created_at, self.extra.get("created_at_timestamp"))?;
        self.updated_at = timestamp(&self.updated_at, self.extra.get("updated_at_timestamp"))?;
        if let Some(key_id) = self.items_key_id {
            self.extra.insert("items_key_id".into(), json!(key_id));
        }
        for name in [
            "enc_item_key",
            "auth_hash",
            "dirty",
            "server_updated_at",
            "errorDecrypting",
        ] {
            self.extra.remove(name);
        }
        Ok(Item {
            uuid: self.uuid,
            content_type: self.content_type,
            content: if self.content.is_null() {
                json!({})
            } else {
                self.content
            },
            created_at: self.created_at,
            server_updated_at: if remote {
                Some(self.updated_at.clone())
            } else {
                None
            },
            updated_at: self.updated_at,
            deleted: self.deleted,
            dirty: !remote,
            extra: self.extra,
        })
    }

    pub fn ksi(&self) -> Option<&str> {
        self.extra
            .get("key_system_identifier")
            .and_then(Value::as_str)
    }
    pub fn shared(&self) -> Option<&str> {
        self.extra.get("shared_vault_uuid").and_then(Value::as_str)
    }

    pub fn for_sync(mut self, item: &Item) -> Result<Value> {
        // The server compares updated_at with its revision. Local edits must not
        // replace this baseline with the local wall clock.
        self.updated_at = item
            .server_updated_at
            .as_ref()
            .unwrap_or(&item.updated_at)
            .clone();
        let created = protocol_timestamp(&self.created_at, item.extra.get("created_at_timestamp"))?;
        let updated = protocol_timestamp(&self.updated_at, item.extra.get("updated_at_timestamp"))?;
        self.extra.retain(|k, _| {
            matches!(
                k.as_str(),
                "duplicate_of" | "key_system_identifier" | "shared_vault_uuid"
            )
        });
        self.extra
            .insert("created_at_timestamp".into(), json!(created));
        self.extra
            .insert("updated_at_timestamp".into(), json!(updated));
        Ok(serde_json::to_value(self)?)
    }
}

fn timestamp(text: &str, fallback: Option<&Value>) -> Result<String> {
    if !text.is_empty() {
        DateTime::parse_from_rfc3339(text).context("Data inválida no item.")?;
        return Ok(text.into());
    }
    if let Some(microseconds) = fallback.and_then(Value::as_i64) {
        return Ok(DateTime::from_timestamp_micros(microseconds)
            .context("Data fora do intervalo.")?
            .to_rfc3339_opts(SecondsFormat::Micros, true));
    }
    Ok("1970-01-01T00:00:00.000Z".into())
}

pub(crate) fn micros(date: &str) -> Result<i64> {
    Ok(DateTime::parse_from_rfc3339(date)
        .context("Data inválida no item.")?
        .timestamp_micros())
}

/// Preserve the server's full microsecond revision, even when its ISO string was
/// rounded to milliseconds by a JavaScript client. Historic local mocks/exports
/// used milliseconds; normalize only when the ISO date proves that relationship.
pub(crate) fn protocol_timestamp(date: &str, stored: Option<&Value>) -> Result<i64> {
    let parsed = DateTime::parse_from_rfc3339(date).context("Data inválida no item.")?;
    match stored.and_then(Value::as_i64) {
        None | Some(0) => Ok(parsed.timestamp_micros()),
        Some(value) if value == parsed.timestamp_millis() => Ok(parsed.timestamp_micros()),
        Some(value) if value >= 0 && value / 1000 == parsed.timestamp_millis() => Ok(value),
        _ => bail!("Timestamp não corresponde à data do item; a revisão original foi preservada."),
    }
}

fn root_encrypted(kind: &str) -> bool {
    matches!(
        kind,
        ITEMS_KEY | VAULT_ROOT | "SN|RootKey|NoSync" | "SN|EncryptedStorage" | "SN|TrustedContact"
    )
}

struct ItemKey {
    key: Zeroizing<String>,
    system: Option<String>,
    default: bool,
    created: i64,
}
struct VaultRoot {
    key: Zeroizing<String>,
    params: Value,
    token: Option<String>,
}

pub(crate) struct KeyRing {
    master: Zeroizing<String>,
    params: Value,
    keys: HashMap<String, ItemKey>,
    vault_roots: HashMap<String, Vec<VaultRoot>>,
}

impl KeyRing {
    pub fn new(root: &RootKey, existing: &[Item]) -> Result<Self> {
        let mut ring = Self {
            master: Zeroizing::new(root.master_key.clone()),
            params: serde_json::to_value(&root.params)?,
            keys: HashMap::new(),
            vault_roots: HashMap::new(),
        };
        for item in existing {
            if !item.deleted {
                ring.remember(&WireItem::from_item(item))?;
            }
        }
        Ok(ring)
    }

    pub fn decrypt_all(&mut self, wire: &[WireItem], remote: bool) -> Result<Vec<Item>> {
        for item in wire {
            uuid::Uuid::parse_str(&item.uuid).context("Identificador de item inválido.")?;
            ensure!(
                !item.content_type.is_empty() && item.content_type.len() <= 256,
                "Tipo de item inválido."
            );
            if remote && !item.deleted {
                // Offline exports may legitimately carry a plaintext items key;
                // a remote sync payload must always authenticate through AEAD.
                // Never let a server-provided JSON object bypass decryption.
                ensure!(
                    item.content.is_string() && item.enc_item_key.is_some(),
                    "Servidor enviou conteúdo não criptografado; nenhum dado local foi alterado."
                );
            }
        }
        let mut output: Vec<Option<Item>> = vec![None; wire.len()];
        let mut indices: Vec<_> = (0..wire.len()).collect();
        indices.sort_by_key(|i| match wire[*i].content_type.as_str() {
            ITEMS_KEY | VAULT_ROOT => 0,
            VAULT_ITEMS_KEY => 1,
            _ => 2,
        });
        for index in indices {
            let decrypted = self
                .decrypt(&wire[index])
                .with_context(|| format!("Não foi possível ler o item {}.", wire[index].uuid))?;
            if !decrypted.deleted {
                self.remember(&decrypted)?;
            }
            output[index] = Some(decrypted.into_item(remote)?);
        }
        output
            .into_iter()
            .map(|v| v.context("Item não processado."))
            .collect()
    }

    fn remember(&mut self, item: &WireItem) -> Result<()> {
        if matches!(item.content_type.as_str(), ITEMS_KEY | VAULT_ITEMS_KEY) {
            // Legacy keys can be preserved in a decrypted backup, but are never
            // selected to encrypt a 004 payload.
            if item.content.get("version").and_then(Value::as_str) != Some("004") {
                return Ok(());
            }
            let key = item
                .content
                .get("itemsKey")
                .and_then(Value::as_str)
                .context("Chave de itens inválida.")?;
            key_bytes(key)?;
            if let Some(existing) = self.keys.get(&item.uuid) {
                ensure!(
                    existing.key.as_str() == key,
                    "Chave imutável foi alterada pelo servidor."
                );
            }
            self.keys.insert(
                item.uuid.clone(),
                ItemKey {
                    key: Zeroizing::new(key.into()),
                    system: item.ksi().map(str::to_owned),
                    default: item
                        .content
                        .get("isDefault")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    created: item
                        .content
                        .get("creationTimestamp")
                        .and_then(Value::as_i64)
                        .or_else(|| micros(&item.created_at).ok())
                        .unwrap_or(0),
                },
            );
        } else if item.content_type == VAULT_ROOT {
            ensure!(
                item.content.get("keyVersion").and_then(Value::as_str) == Some("004"),
                "Versão de cofre não suportada."
            );
            let id = item
                .content
                .get("systemIdentifier")
                .and_then(Value::as_str)
                .context("Identificador de cofre ausente.")?;
            let key = item
                .content
                .get("key")
                .and_then(Value::as_str)
                .context("Chave de cofre ausente.")?;
            key_bytes(key)?;
            self.vault_roots
                .entry(id.into())
                .or_default()
                .push(VaultRoot {
                    key: Zeroizing::new(key.into()),
                    params: item
                        .content
                        .get("keyParams")
                        .cloned()
                        .context("Parâmetros de cofre ausentes.")?,
                    token: item
                        .content
                        .get("token")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                });
        }
        Ok(())
    }

    pub fn default_key_id(&self, system: Option<&str>) -> Option<&str> {
        self.keys
            .iter()
            .filter(|(_, key)| key.system.as_deref() == system)
            .max_by(|a, b| (a.1.default, a.1.created, a.0).cmp(&(b.1.default, b.1.created, b.0)))
            .map(|(id, _)| id.as_str())
    }

    fn decrypt(&self, item: &WireItem) -> Result<WireItem> {
        ensure!(
            item.shared().is_none(),
            "Cofres compartilhados ainda não são suportados; nenhuma alteração foi aplicada."
        );
        if item.deleted || item.content.is_object() {
            return Ok(item.clone());
        }
        let encrypted = item
            .content
            .as_str()
            .context("Conteúdo criptografado ausente.")?;
        if root_encrypted(&item.content_type) {
            return self.decrypt_using(item, encrypted, &self.master);
        }
        if item.content_type == VAULT_ITEMS_KEY {
            let system = item.ksi().context("Identificador de cofre ausente.")?;
            if let Some(roots) = self.vault_roots.get(system) {
                for root in roots {
                    if let Ok(decrypted) = self.decrypt_using(item, encrypted, &root.key) {
                        return Ok(decrypted);
                    }
                }
            }
            bail!(
                "Chave do cofre privado indisponível; abra e exporte esse cofre no aplicativo original."
            );
        }
        let key_id = item
            .items_key_id
            .as_ref()
            .context("Item sem identificador de chave.")?;
        let key = self
            .keys
            .get(key_id)
            .context("Chave do item indisponível; backup incompleto ou formato legado.")?;
        ensure!(
            key.system.as_deref() == item.ksi(),
            "Chave pertence a outro cofre."
        );
        self.decrypt_using(item, encrypted, &key.key)
    }

    fn decrypt_using(&self, item: &WireItem, encrypted: &str, key: &str) -> Result<WireItem> {
        let enc_key = item
            .enc_item_key
            .as_deref()
            .context("Chave de conteúdo ausente.")?;
        let content_key = decrypt_string(enc_key, key, &item.uuid, item.ksi(), item.shared())?;
        let plaintext = decrypt_string(
            encrypted,
            &content_key,
            &item.uuid,
            item.ksi(),
            item.shared(),
        )?;
        let content: Value = serde_json::from_str(&plaintext).context("JSON do item inválido.")?;
        ensure!(content.is_object(), "Conteúdo do item não é um objeto.");
        let mut result = item.clone();
        result.content = content;
        result.enc_item_key = None;
        Ok(result)
    }

    pub fn encrypt(&self, item: &Item) -> Result<WireItem> {
        let mut wire = WireItem::from_item(item);
        ensure!(
            wire.shared().is_none(),
            "Edição de cofres compartilhados exige o aplicativo original."
        );
        if wire.deleted {
            wire.content = Value::Null;
            return Ok(wire);
        }
        ensure!(
            wire.content.is_object(),
            "Conteúdo inválido para criptografia."
        );
        let (key, params, selected_id): (&str, Option<&Value>, Option<&str>) =
            if root_encrypted(&wire.content_type) {
                (&self.master, Some(&self.params), None)
            } else if wire.content_type == VAULT_ITEMS_KEY {
                let system = wire.ksi().context("Identificador de cofre ausente.")?;
                let token = wire.content.get("rootKeyToken").and_then(Value::as_str);
                let root = self
                    .vault_roots
                    .get(system)
                    .and_then(|r| r.iter().find(|k| k.token.as_deref() == token))
                    .context("Chave privada indisponível.")?;
                (&root.key, Some(&root.params), None)
            } else {
                let id = wire
                    .items_key_id
                    .as_deref()
                    .filter(|id| {
                        self.keys
                            .get(*id)
                            .is_some_and(|k| k.system.as_deref() == wire.ksi())
                    })
                    .or_else(|| self.default_key_id(wire.ksi()))
                    .context("Nenhuma chave de itens 004 disponível.")?;
                (
                    &self.keys.get(id).context("Chave indisponível.")?.key,
                    None,
                    Some(id),
                )
            };
        let aad = aad_for(&wire.uuid, params, wire.ksi());
        let key_id = selected_id.map(str::to_owned);
        let mut plaintext = serde_json::to_string(&wire.content)?;
        let content_key = Zeroizing::new(random_hex(32));
        wire.enc_item_key = Some(encrypt_string(&content_key, key, &aad)?);
        let encrypted = encrypt_string(&plaintext, &content_key, &aad)?;
        plaintext.zeroize();
        wire.content = json!(encrypted);
        wire.items_key_id = key_id;
        Ok(wire)
    }

    pub fn add_new_items_key(&mut self) -> Result<Item> {
        let mut key = Item::new_note("", "");
        key.content_type = ITEMS_KEY.into();
        key.content = json!({"itemsKey":random_hex(32),"version":"004","isDefault":true,"creationTimestamp":Utc::now().timestamp_millis(),"references":[],"appData":{}});
        self.remember(&WireItem::from_item(&key))?;
        Ok(key)
    }
}
