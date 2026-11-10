use serde_json::{Value, json};
use sn_rune_core::{
    APP_DOMAIN, BackupDocument, Item, VaultData, export_backup, export_document, import_backup,
    import_document, import_file, merge_document, merge_import, set_note_tags, tag_names,
};
use std::collections::BTreeMap;

fn sample() -> Value {
    json!({
        "version": "004",
        "custom_exporter": {"version": 7, "opaque": ["ação", false, {"future": null}]},
        "items": [{
            "uuid": "01234567-89ab-4cde-8012-3456789abcde",
            "content_type": "Note",
            "created_at": "2024-01-01T00:00:00.000Z",
            "updated_at": "2024-02-01T12:34:56.789Z",
            "content": {
                "title": "João — 你好",
                "text": "Linha 1\r\nLinha 2\nÚltima\t📝",
                "references": [{"uuid": "11111111-2222-4333-8444-555555555555", "content_type": "Note", "reference_type": "future-type", "opaque": 9}],
                "appData": {"org.standardnotes.sn": {"pinned": true}, "third.party": {"nested": [1,2,3]}},
                "unknown_content": {"deep": [null, "keep"]}
            },
            "future_field": {"decimal": 12.75, "array": [1, false, "keep"]},
            "created_at_timestamp": 1704067200000000i64,
            "updated_at_timestamp": 1706790896789000i64,
            "shared_vault_uuid": "22222222-2222-4222-8222-222222222222"
        }]
    })
}

#[test]
fn standardnotes_envelope_and_unknown_item_fields_survive_roundtrip() {
    let original = sample();
    let document = import_document(&original.to_string()).unwrap();
    let output: Value = serde_json::from_str(&export_document(&document).unwrap()).unwrap();
    assert_eq!(output["custom_exporter"], original["custom_exporter"]);
    let item = &output["items"][0];
    for (key, expected) in original["items"][0].as_object().unwrap() {
        assert_eq!(&item[key], expected, "field {key} changed");
    }
    assert_eq!(document.items[0].text(), "Linha 1\r\nLinha 2\nÚltima\t📝");
}

#[test]
fn editing_preserves_server_timestamp_and_foreign_metadata() {
    let mut note = import_backup(&sample().to_string()).unwrap().remove(0);
    let before = note.clone();
    note.server_updated_at = Some(before.updated_at.clone());
    note.extra
        .insert("signatureData".into(), json!({"signature": "obsolete"}));
    note.set_title("Novo título").unwrap();
    note.set_text("Novo conteúdo").unwrap();
    assert_eq!(note.updated_at, before.updated_at);
    assert_eq!(
        note.extra["updated_at_timestamp"],
        before.extra["updated_at_timestamp"]
    );
    assert_eq!(
        note.content["appData"]["third.party"],
        before.content["appData"]["third.party"]
    );
    assert_eq!(note.content["references"], before.content["references"]);
    assert_eq!(note.extra["future_field"], before.extra["future_field"]);
    assert!(note.dirty);
    assert!(!note.extra.contains_key("signatureData"));
    assert!(note.content["appData"][APP_DOMAIN]["client_updated_at"].is_string());
}

#[test]
fn unknown_rich_editor_is_preserved_and_cannot_be_saved_as_plaintext() {
    let mut note = Item::new_note("Super", "{\"root\":{\"children\":[]}}");
    note.content["noteType"] = json!("super");
    note.content["editorIdentifier"] = json!("org.standardnotes.super-editor");
    let original = note.clone();
    assert!(!note.is_editable_plain_text());
    assert!(note.set_text("would destroy rich structure").is_err());
    assert_eq!(note, original);
    let reparsed = import_backup(&export_backup(&[note]).unwrap()).unwrap();
    assert_eq!(reparsed[0], original);
}

#[test]
fn flags_use_upstream_app_data_layout() {
    let mut note = Item::new_note("A", "B");
    note.set_flag("pinned", true).unwrap();
    note.set_flag("archived", true).unwrap();
    note.set_flag("trashed", true).unwrap();
    assert_eq!(note.content["appData"][APP_DOMAIN]["pinned"], true);
    assert_eq!(note.content["appData"][APP_DOMAIN]["archived"], true);
    assert_eq!(note.content["trashed"], true);
    assert!(note.content.get("pinned").is_none());
    assert!(note.flag("pinned") && note.flag("archived") && note.flag("trashed"));
}

#[test]
fn malformed_app_data_is_not_overwritten_by_an_edit() {
    let mut note = Item::new_note("A", "B");
    note.content["appData"] = json!("opaque future format");
    let original = note.clone();
    assert!(note.set_text("new").is_err());
    assert!(note.set_flag("pinned", true).is_err());
    assert_eq!(note, original);
}

#[test]
fn encrypted_backups_are_rejected_as_a_whole() {
    let plain = Item::new_note("Plain", "value");
    let mut raw = serde_json::to_value(&plain).unwrap();
    raw["content"] = json!("004:nonce:payload:ad");
    assert!(import_backup(&json!({"items": [plain, raw]}).to_string()).is_err());
    assert!(
        import_backup(&json!({"keyParams": {"version":"004"}, "items": []}).to_string()).is_err()
    );
    assert!(
        import_backup(&json!({"auth_params": {"version":"003"}, "items": []}).to_string()).is_err()
    );
}

#[test]
fn import_rejects_invalid_ids_and_duplicate_ids() {
    let note = Item::new_note("A", "B");
    assert!(import_backup(&json!({"items": [note.clone(), note]}).to_string()).is_err());
    let mut original = sample();
    original["items"][0]["uuid"] = json!("not-a-uuid");
    assert!(import_backup(&original.to_string()).is_err());
}

#[test]
fn conflict_import_preserves_existing_item_and_retargets_imported_tags() {
    let note = Item::new_note("Local", "local content");
    let mut imported = note.clone();
    imported.set_text("imported content").unwrap();
    let mut tag = Item::new_tag("Trabalho");
    tag.content["references"] =
        json!([{"uuid": note.uuid, "content_type":"Note", "extra": "preserve"}]);
    let mut data = VaultData {
        items: vec![note.clone()],
        ..VaultData::default()
    };
    let report = merge_import(&mut data, vec![imported, tag]).unwrap();
    assert_eq!((report.added, report.skipped, report.conflicts), (2, 0, 1));
    assert_eq!(data.items[0], note);
    let copy = data
        .items
        .iter()
        .find(|item| item.is_note() && item.uuid != note.uuid)
        .unwrap();
    assert_eq!(copy.text(), "imported content");
    assert_eq!(copy.extra["duplicate_of"], note.uuid);
    let tag = data.items.iter().find(|item| item.is_tag()).unwrap();
    assert_eq!(tag.content["references"][0]["uuid"], copy.uuid);
    assert_eq!(tag.content["references"][0]["extra"], "preserve");
}

#[test]
fn existing_equal_tag_is_duplicated_when_its_imported_reference_changes() {
    let local = Item::new_note("A", "local");
    let mut remote = local.clone();
    remote.set_text("incoming").unwrap();
    let mut tag = Item::new_tag("Tag");
    tag.content["references"] = json!([{"uuid":local.uuid,"content_type":"Note"}]);
    let mut data = VaultData {
        items: vec![local.clone(), tag.clone()],
        ..VaultData::default()
    };
    let report = merge_import(&mut data, vec![remote, tag.clone()]).unwrap();
    assert_eq!(report.conflicts, 2);
    assert_eq!(data.items[0], local);
    assert_eq!(data.items[1], tag);
    assert_eq!(data.items.len(), 4);
}

#[test]
fn unchanged_import_is_idempotent_and_invalid_batch_is_atomic() {
    let note = Item::new_note("A", "B");
    let mut data = VaultData {
        items: vec![note.clone()],
        ..VaultData::default()
    };
    let report = merge_import(&mut data, vec![note]).unwrap();
    assert_eq!((report.added, report.skipped, report.conflicts), (0, 1, 0));
    let original = data.clone();
    let good = Item::new_note("good", "content");
    let mut bad = Item::new_note("bad", "content");
    bad.uuid = "invalid".into();
    assert!(merge_import(&mut data, vec![good, bad]).is_err());
    assert_eq!(data, original);
}

#[test]
fn setting_tags_preserves_other_relationships_and_foreign_fields() {
    let note = Item::new_note("A", "B");
    let mut tag = Item::new_tag("Velha");
    let other_id = "11111111-2222-4333-8444-555555555555";
    tag.content["references"] = json!([
        {"uuid":note.uuid,"content_type":"Note"},
        {"uuid":other_id,"content_type":"SN|File","opaque":"stay"}
    ]);
    tag.content["custom"] = json!({"field":true});
    let mut data = VaultData {
        items: vec![note.clone(), tag],
        ..VaultData::default()
    };
    set_note_tags(
        &mut data,
        &note.uuid,
        &[" Nova ".into(), "Nova".into(), "".into()],
    )
    .unwrap();
    assert_eq!(tag_names(&data, &note.uuid), vec!["Nova"]);
    let old = data
        .items
        .iter()
        .find(|item| item.title() == "Velha")
        .unwrap();
    assert_eq!(
        old.content["references"],
        json!([{"uuid":other_id,"content_type":"SN|File","opaque":"stay"}])
    );
    assert_eq!(old.content["custom"], json!({"field":true}));
}

#[test]
fn text_import_keeps_markdown_and_unicode_exactly() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Título açãõ.md");
    let content = "# Título\n\n- [x] teste\r\n<literal>\t你好";
    std::fs::write(&path, content).unwrap();
    let items = import_file(&path).unwrap();
    assert_eq!(items[0].title(), "Título açãõ");
    assert_eq!(items[0].text(), content);
    assert!(items[0].is_editable_plain_text());
}

#[test]
fn item_content_types_and_envelope_are_not_filtered() {
    let mut opaque = Item::new_note("Unknown", "opaque");
    opaque.content_type = "future|unknown-item".into();
    let document = BackupDocument {
        items: vec![opaque.clone()],
        metadata: BTreeMap::from([("future".into(), json!({"keep": true}))]),
    };
    let output = import_document(&export_document(&document).unwrap()).unwrap();
    assert_eq!(output.items[0], opaque);
    assert_eq!(output.metadata["future"], json!({"keep":true}));
}

#[test]
fn clearing_sensitive_state_removes_owned_data() {
    let mut data = VaultData {
        items: vec![Item::new_note("Secret title", "Secret note")],
        account: Some(json!({"session": "secret-token"})),
        ..VaultData::default()
    };
    data.settings.insert("private".into(), json!(["value"]));
    data.clear_sensitive();
    assert_eq!(data, VaultData::default());
}

#[test]
fn multiple_source_envelopes_with_conflicting_fields_are_retained() {
    let mut data = VaultData::default();
    merge_document(
        &mut data,
        BackupDocument {
            items: vec![Item::new_note("First", "A")],
            metadata: BTreeMap::from([("custom_exporter".into(), json!({"version":1}))]),
        },
    )
    .unwrap();
    merge_document(
        &mut data,
        BackupDocument {
            items: vec![Item::new_note("Second", "B")],
            metadata: BTreeMap::from([
                ("custom_exporter".into(), json!({"version":2})),
                ("new_field".into(), json!(true)),
            ]),
        },
    )
    .unwrap();
    assert_eq!(
        data.backup_metadata["custom_exporter"],
        json!({"version":1})
    );
    assert_eq!(data.backup_metadata["new_field"], true);
    let exported = export_document(&BackupDocument {
        items: data.items,
        metadata: data.backup_metadata,
    })
    .unwrap();
    let parsed: Value = serde_json::from_str(&exported).unwrap();
    assert_eq!(
        parsed["_standardnotes_rune_source_envelopes"][0]["custom_exporter"],
        json!({"version":2})
    );
}

#[test]
fn plaintext_export_excludes_account_and_item_key_material() {
    let note = Item::new_note("Exportable", "Note content remains");
    let mut data = VaultData {
        items: vec![note],
        account: Some(
            json!({"master_key":"ACCOUNT_KEY_MUST_NOT_APPEAR", "session":"SESSION_MUST_NOT_APPEAR"}),
        ),
        ..VaultData::default()
    };
    for kind in [
        "SN|ItemsKey",
        "SN|RootKey|NoSync",
        "SN|EncryptedStorage",
        "SN|KeySystemRootKey",
        "SN|KeySystemItemsKey",
    ] {
        let mut key = Item::new_note("internal", "");
        key.content_type = kind.to_owned();
        key.content =
            json!({"itemsKey":"ITEM_KEY_MUST_NOT_APPEAR", "key":"ROOT_KEY_MUST_NOT_APPEAR"});
        data.items.push(key);
    }
    let exported = export_backup(&data.items).unwrap();
    assert!(!exported.contains("MUST_NOT_APPEAR"));
    let parsed = import_backup(&exported).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].text(), "Note content remains");
    // Filtering a plaintext export must not remove anything from the vault.
    assert_eq!(data.items.len(), 6);
    assert!(data.account.is_some());
}

#[test]
fn unfamiliar_large_json_numbers_are_preserved_exactly() {
    let raw = r#"{"items":[{"uuid":"01234567-89ab-4cde-8012-3456789abcde","content_type":"Future","content":{"opaque":1234567890123456789012345678901234567890},"extra_number":1.23456789012345678901234567890}] }"#;
    let document = import_document(raw).unwrap();
    let exported = export_document(&document).unwrap();
    assert!(exported.contains("1234567890123456789012345678901234567890"));
    assert!(exported.contains("1.23456789012345678901234567890"));
}

#[test]
fn markdown_and_code_edit_the_source_without_changing_editor_metadata() {
    for (kind, editor) in [
        ("markdown", "org.standardnotes.advanced-markdown-editor"),
        ("markdown", "org.standardnotes.markdown-visual-editor"),
        ("markdown", "org.standardnotes.simple-markdown-editor"),
        ("markdown", "org.standardnotes.fancy-markdown-editor"),
        ("markdown", "org.standardnotes.minimal-markdown-editor"),
        ("code", "org.standardnotes.code-editor"),
    ] {
        let mut note = Item::new_note("Source", "original source");
        note.content["noteType"] = json!(kind);
        note.content["editorIdentifier"] = json!(editor);
        note.content["editor_state"] = json!({"language":"rust", "math":true});
        assert!(note.is_editable_text_source());
        assert!(!note.is_editable_plain_text());
        note.set_text("# Alterado\n\n```rust\nfn main() {}\n```\n$E=mc^2$")
            .unwrap();
        let exported = export_backup(&[note.clone()]).unwrap();
        let restored = import_backup(&exported).unwrap().remove(0);
        assert_eq!(restored.content["noteType"], kind);
        assert_eq!(restored.content["editorIdentifier"], editor);
        assert_eq!(
            restored.content["editor_state"],
            json!({"language":"rust", "math":true})
        );
        assert_eq!(restored.text(), note.text());
        // Legacy documents predate noteType but still use the same textual source.
        note.content.as_object_mut().unwrap().remove("noteType");
        assert!(note.is_editable_text_source());
        note.set_text("new legacy source").unwrap();
        assert!(note.content.get("noteType").is_none());
        assert_eq!(note.content["editorIdentifier"], editor);
    }
    for kind in [
        "super",
        "rich-text",
        "spreadsheet",
        "task",
        "authentication",
        "unknown",
    ] {
        let mut note = Item::new_note("Opaque", "preserve");
        note.content["noteType"] = json!(kind);
        assert!(!note.is_editable_text_source());
        assert!(note.set_text("destructive").is_err());
        assert_eq!(note.text(), "preserve");
    }
    let mut unfamiliar = Item::new_note("Custom", "preserve");
    unfamiliar.content["noteType"] = json!("markdown");
    unfamiliar.content["editorIdentifier"] = json!("third.party.unknown");
    assert!(!unfamiliar.is_editable_text_source());
}
