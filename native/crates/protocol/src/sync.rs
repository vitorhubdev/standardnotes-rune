use crate::account::{API_VERSION, AccountSession, Api};
use crate::wire::{ITEMS_KEY, KeyRing, WireItem, micros, protocol_timestamp};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sn_rune_core::{Item, VaultData};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const BATCH_SIZE: usize = 150;
const MAX_ITEMS: usize = 250_000;
const MAX_PAGES: usize = 10_000;

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SyncReport {
    pub downloaded: usize,
    pub uploaded: usize,
    pub conflicts: usize,
    pub pending: usize,
}

/// Synchronize private protocol-004 items with the configured account.
///
/// Incoming data, cursor advancement and dirty flags are staged and committed
/// together. No local item changes on failure. A failed HTTP write may already
/// exist remotely; the next download compares decrypted content to reconcile it.
/// Refresh tokens can rotate before a later failure: callers MUST persist the
/// supplied `session` inside their encrypted vault even when this returns Err.
/// Account data should always be persisted together with the returned vault.
pub fn sync(session: &mut AccountSession, data: &mut VaultData) -> Result<SyncReport> {
    let api = Api::new(&session.server)?;
    let mut staged = data.clone();
    let mut report = SyncReport::default();
    let mut token = session.sync_token.clone();
    let mut ring = KeyRing::new(&session.root, &staged.items)?;

    // Download and authenticate every page before sending any changes. A legacy
    // or shared item therefore cannot cause a partially migrated first upload.
    let mut initial = Vec::new();
    pull_pages(&api, session, &mut token, None, &mut initial)?;
    let incoming = ring.decrypt_all(&last_revisions(initial), true)?;
    report.downloaded += incoming.len();
    merge_remote(&mut staged, incoming, &mut report)?;
    ring = KeyRing::new(&session.root, &staged.items)?;

    if !session.tokens.readonly_access {
        let any_regular_dirty = staged
            .items
            .iter()
            .any(|i| i.dirty && !i.deleted && !local_only(i));
        if any_regular_dirty && ring.default_key_id(None).is_none() {
            staged.items.push(ring.add_new_items_key()?);
        }
    }

    let mut pending: Vec<Item> = staged
        .items
        .iter()
        .filter(|i| i.dirty && !local_only(i))
        .cloned()
        .collect();
    pending.sort_by_key(|item| if item.content_type == ITEMS_KEY { 0 } else { 1 });
    if session.tokens.readonly_access {
        report.pending = pending.len();
        session.sync_token = token;
        staged.account = Some(session.to_vault_value()?);
        *data = staged;
        return Ok(report);
    }

    // Materialize all encrypted requests before the first upload, so a missing
    // key/unsupported item cannot fail half way through request construction.
    for item in &pending {
        ring.encrypt(item)?.for_sync(item)?;
    }
    for planned in pending.chunks(BATCH_SIZE) {
        // Previous responses may have resolved a conflict or updated a tag that
        // belongs to a later batch. Re-read current staged items so stale copies
        // are never uploaded over a newly downloaded revision.
        let items: Vec<Item> = planned
            .iter()
            .filter_map(|planned| {
                staged
                    .items
                    .iter()
                    .find(|i| i.uuid == planned.uuid && i.dirty)
            })
            .cloned()
            .collect();
        if items.is_empty() {
            continue;
        }
        let encrypted: Vec<Value> = items
            .iter()
            .map(|item| ring.encrypt(item)?.for_sync(item))
            .collect::<Result<_>>()?;
        let body = request(&token, None, &encrypted);
        let response = api.authenticated_post("/v1/items", &body, session)?;
        check_auxiliary(&response)?;
        let new_token = response_token(&response, &token)?;
        let cursor = optional_string(&response, "cursor_token")?;
        let saved = get_items(&response, "saved_items")?;
        let mut received = get_items(&response, "retrieved_items")?;
        let (conflicts, rejected) = parse_conflicts(&response)?;
        received.extend(conflicts);
        let expected: BTreeSet<_> = items.iter().map(|i| i.uuid.as_str()).collect();
        let saved_ids: BTreeSet<_> = saved.iter().map(|i| i.uuid.as_str()).collect();
        ensure!(
            saved_ids.is_subset(&expected),
            "Servidor confirmou um item que não foi enviado."
        );
        for id in &expected {
            ensure!(
                saved_ids.contains(id) || rejected.contains(*id),
                "Servidor não confirmou todos os itens; alterações continuam pendentes."
            );
        }
        ensure!(
            saved_ids.iter().all(|id| !rejected.contains(*id)),
            "Servidor retornou confirmação e conflito para o mesmo item."
        );

        // Acknowledgements contain server timestamps but may omit content.
        // Keep our plaintext and mark clean only IDs explicitly acknowledged.
        for saved in &saved {
            let local = staged
                .items
                .iter_mut()
                .find(|i| i.uuid == saved.uuid)
                .context("Confirmação desconhecida.")?;
            let revision = normalized_saved_time(saved)?;
            local.updated_at = revision.clone();
            local.server_updated_at = Some(revision);
            local.dirty = false;
            if let Some(timestamp) = saved.extra.get("updated_at_timestamp") {
                local
                    .extra
                    .insert("updated_at_timestamp".into(), timestamp.clone());
            } else {
                // Do not retain a numeric revision belonging to the previous
                // server timestamp if an older server only returned ISO dates.
                local.extra.insert(
                    "updated_at_timestamp".into(),
                    json!(micros(&local.updated_at)?),
                );
            }
            report.uploaded += 1;
        }
        token = new_token;
        if cursor.is_some() {
            pull_pages(&api, session, &mut token, cursor, &mut received)?;
        }
        let received = ring.decrypt_all(&last_revisions(received), true)?;
        report.downloaded += received.len();
        merge_remote(&mut staged, received, &mut report)?;
    }
    report.pending = staged
        .items
        .iter()
        .filter(|i| i.dirty && !local_only(i))
        .count();
    session.sync_token = token;
    staged.account = Some(session.to_vault_value()?);
    *data = staged;
    Ok(report)
}

fn local_only(item: &Item) -> bool {
    matches!(
        item.content_type.as_str(),
        "SN|RootKey|NoSync" | "SN|EncryptedStorage"
    )
}

fn request(token: &Option<String>, cursor: Option<&str>, items: &[Value]) -> Value {
    let mut value = json!({"api":API_VERSION,"limit":BATCH_SIZE,"items":items});
    if let Some(token) = token {
        value["sync_token"] = json!(token);
    }
    if let Some(cursor) = cursor {
        value["cursor_token"] = json!(cursor);
    }
    value
}

fn pull_pages(
    api: &Api,
    session: &mut AccountSession,
    token: &mut Option<String>,
    mut cursor: Option<String>,
    items: &mut Vec<WireItem>,
) -> Result<()> {
    let mut cursors = HashSet::new();
    for _ in 0..MAX_PAGES {
        if let Some(cursor) = &cursor {
            ensure!(
                cursors.insert(cursor.clone()),
                "Servidor repetiu a página de sincronização."
            );
        }
        let response = api.authenticated_post(
            "/v1/items",
            &request(token, cursor.as_deref(), &[]),
            session,
        )?;
        check_auxiliary(&response)?;
        ensure!(
            get_items(&response, "saved_items")?.is_empty(),
            "Servidor confirmou alterações sem envio."
        );
        let (conflicts, rejected) = parse_conflicts(&response)?;
        ensure!(
            conflicts.is_empty() && rejected.is_empty(),
            "Servidor retornou conflitos durante leitura."
        );
        items.extend(get_items(&response, "retrieved_items")?);
        ensure!(
            items.len() <= MAX_ITEMS,
            "Sincronização excedeu 250.000 itens; os dados locais foram preservados."
        );
        *token = response_token(&response, token)?;
        cursor = optional_string(&response, "cursor_token")?;
        if cursor.is_none() {
            return Ok(());
        }
    }
    bail!("Sincronização excedeu o limite de páginas.");
}

fn optional_string(response: &Value, key: &str) -> Result<Option<String>> {
    match response.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) if s.len() <= 64 * 1024 => Ok(Some(s.clone())),
        _ => bail!("Token de sincronização inválido."),
    }
}

fn response_token(response: &Value, old: &Option<String>) -> Result<Option<String>> {
    let token = optional_string(response, "sync_token")?;
    ensure!(
        token.is_some(),
        "Servidor não retornou um token de sincronização; os dados locais foram preservados."
    );
    Ok(token.or_else(|| old.clone()))
}

fn check_auxiliary(response: &Value) -> Result<()> {
    for key in ["shared_vaults", "shared_vault_invites", "messages"] {
        if let Some(value) = response.get(key) {
            ensure!(
                value.is_null() || value.is_array(),
                "Resposta de sincronização inválida."
            );
            ensure!(
                value.as_array().is_none_or(Vec::is_empty),
                "A conta usa cofres compartilhados; a sincronização completa requer o aplicativo original."
            );
        }
    }
    Ok(())
}

fn get_items(response: &Value, name: &str) -> Result<Vec<WireItem>> {
    let Some(value) = response.get(name) else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let values = value.as_array().context("Lista de itens inválida.")?;
    ensure!(values.len() <= MAX_ITEMS, "Servidor retornou itens demais.");
    serde_json::from_value(value.clone()).context("Estrutura de item recebida inválida.")
}

fn parse_conflicts(response: &Value) -> Result<(Vec<WireItem>, BTreeSet<String>)> {
    let mut server_items = Vec::new();
    let mut rejected = BTreeSet::new();
    for key in ["conflicts", "unsaved"] {
        let Some(values) = response.get(key).filter(|v| !v.is_null()) else {
            continue;
        };
        for conflict in values.as_array().context("Lista de conflitos inválida.")? {
            let unsaved: WireItem = serde_json::from_value(
                conflict
                    .get("unsaved_item")
                    .cloned()
                    .context("Conflito sem item original.")?,
            )?;
            let server: WireItem = serde_json::from_value(
                conflict
                    .get("server_item")
                    .cloned()
                    .context("Item rejeitado pelo servidor; nenhuma alteração local aplicada.")?,
            )?;
            ensure!(
                server.uuid == unsaved.uuid,
                "Conflito com identificadores diferentes."
            );
            rejected.insert(unsaved.uuid);
            server_items.push(server);
        }
    }
    Ok((server_items, rejected))
}

fn normalized_saved_time(item: &WireItem) -> Result<String> {
    if !item.updated_at.is_empty() {
        micros(&item.updated_at)?;
        return Ok(item.updated_at.clone());
    }
    let timestamp = item
        .extra
        .get("updated_at_timestamp")
        .and_then(Value::as_i64)
        .context("Confirmação sem revisão do servidor.")?;
    Ok(chrono::DateTime::from_timestamp_micros(timestamp)
        .context("Revisão do servidor inválida.")?
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true))
}

fn last_revisions(items: Vec<WireItem>) -> Vec<WireItem> {
    // Later pages may include a newer revision of an already retrieved UUID.
    // BTreeMap yields a deterministic order; KeyRing subsequently sorts keys first.
    let mut latest: BTreeMap<String, WireItem> = BTreeMap::new();
    for item in items {
        latest.insert(item.uuid.clone(), item);
    }
    latest.into_values().collect()
}

pub(crate) fn merge_remote(
    data: &mut VaultData,
    incoming: Vec<Item>,
    report: &mut SyncReport,
) -> Result<()> {
    let mut positions: HashMap<_, _> = data
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.uuid.clone(), i))
        .collect();
    for remote in incoming {
        let Some(&index) = positions.get(&remote.uuid) else {
            positions.insert(remote.uuid.clone(), data.items.len());
            data.items.push(remote);
            continue;
        };
        let local = &data.items[index];
        if !local.dirty || same_content(local, &remote) {
            let mut replacement = remote;
            // Keep a local recovery copy of content for a tombstone. It is never
            // uploaded in the tombstone payload and remains inside the vault.
            if replacement.deleted && replacement.content.as_object().is_none_or(|o| o.is_empty()) {
                replacement.content = local.content.clone();
            }
            data.items[index] = replacement;
            continue;
        }
        let base = local
            .server_updated_at
            .as_deref()
            .unwrap_or(&local.updated_at);
        if protocol_timestamp(base, local.extra.get("updated_at_timestamp"))?
            == protocol_timestamp(&remote.updated_at, remote.extra.get("updated_at_timestamp"))?
        {
            // The server sent the revision we edited; retain the local change
            // and its baseline for the later conditional save.
            continue;
        }
        ensure!(
            local.content_type == remote.content_type,
            "Tipo do item mudou durante a sincronização."
        );
        ensure!(
            local.content_type == "Note" && !local.deleted,
            "Conflito em item que exige resolução manual; nenhuma alteração local foi aplicada."
        );
        let mut duplicate = local.clone();
        duplicate.uuid = uuid::Uuid::new_v4().to_string();
        duplicate.updated_at = "1970-01-01T00:00:00.000Z".into();
        duplicate.server_updated_at = None;
        duplicate
            .extra
            .insert("duplicate_of".into(), json!(local.uuid));
        duplicate.extra.remove("updated_at_timestamp");
        duplicate.extra.remove("signatureData");
        duplicate.extra.remove("last_edited_by_uuid");
        duplicate.content["conflict_of"] = json!(local.uuid);
        // Keep the original title/content unchanged. The conflict marker is a
        // separate field that Standard Notes understands; never rewrite prose.
        duplicate.dirty = true;
        let local_id = local.uuid.clone();
        let duplicate_id = duplicate.uuid.clone();
        data.items[index] = remote;
        positions.insert(duplicate.uuid.clone(), data.items.len());
        data.items.push(duplicate);
        preserve_tag_memberships(&mut data.items, &local_id, &duplicate_id)?;
        report.conflicts += 1;
    }
    Ok(())
}

fn preserve_tag_memberships(items: &mut [Item], original: &str, duplicate: &str) -> Result<()> {
    for tag in items
        .iter_mut()
        .filter(|i| !i.deleted && i.content_type == "Tag")
    {
        let Some(references) = tag
            .content
            .get_mut("references")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        if references
            .iter()
            .any(|reference| reference.get("uuid").and_then(Value::as_str) == Some(original))
        {
            references.push(json!({"uuid":duplicate,"content_type":"Note"}));
            tag.touch()?;
        }
    }
    Ok(())
}

fn same_content(a: &Item, b: &Item) -> bool {
    a.content_type == b.content_type
        && a.deleted == b.deleted
        && (a.deleted || a.content == b.content)
        && ["key_system_identifier", "shared_vault_uuid", "duplicate_of"]
            .iter()
            .all(|key| {
                a.extra.get(*key).filter(|v| !v.is_null())
                    == b.extra.get(*key).filter(|v| !v.is_null())
            })
}
