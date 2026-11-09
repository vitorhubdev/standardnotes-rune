use std::time::{Duration, Instant};

use rune::{Env, Key, KeyEvent, Mods, Runtime, TextEdit};
use sn_rune_core::{Item, Vault};
use zeroize::Zeroizing;

use crate::state::{AppState, Filter, SavePhase, Session, editable};
use crate::worker::{Command, Event};

fn runtime() -> Runtime<AppState> {
    let mut state = AppState::at("unused-demo-vault".into(), true);
    state.demo_scene();
    let mut runtime = Runtime::new(
        state,
        crate::views::view,
        (1200.0, 800.0),
        Env::new(true, 1.0),
    );
    runtime.on_key = Some(crate::shortcuts);
    runtime.frame(0.0);
    runtime
}

fn tap(runtime: &mut Runtime<AppState>, key: &str) {
    // Kit buttons entering a new view fade in. Hit-testing intentionally ignores
    // a fully transparent node, so let the same animation as the desktop settle.
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
    let (x, y) = runtime
        .find(key)
        .unwrap_or_else(|| panic!("element {key} missing"));
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
}

fn wait_event(state: &AppState) -> Event {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(event) = state.worker.try_recv() {
            return event;
        }
        assert!(Instant::now() < deadline, "worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn clicking_new_typing_and_trashing_uses_real_handlers() {
    let mut runtime = runtime();
    let before = runtime.state.open().unwrap().data.items.len();
    tap(&mut runtime, "new-note");
    assert_eq!(runtime.state.open().unwrap().data.items.len(), before + 1);
    // New focuses the title field through the same Tx request as keyboard Ctrl+N.
    runtime.key(KeyEvent {
        key: Key::Text("Título escrito".into()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(runtime.state.selected().unwrap().title(), "Título escrito");
    tap(&mut runtime, "note-body/scroll/input");
    runtime.key(KeyEvent {
        key: Key::Text("Uma nota com acentuação: ação.".into()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert_eq!(
        runtime.state.selected().unwrap().text(),
        "Uma nota com acentuação: ação."
    );
    tap(&mut runtime, "toggle-trash");
    assert!(runtime.state.selected().unwrap().flag("trashed"));
    tap(&mut runtime, "filter-trash");
    assert_eq!(runtime.state.open().unwrap().visible.len(), 1);
    tap(&mut runtime, "toggle-trash");
    assert!(!runtime.state.selected().unwrap().flag("trashed"));
}

#[test]
fn a_saved_old_revision_does_not_mark_a_new_edit_saved() {
    let mut runtime = runtime();
    runtime.state.demo = false;
    runtime.state.open_mut().unwrap().revision = 4;
    runtime.state.open_mut().unwrap().save = SavePhase::Writing { revision: 4 };
    runtime.update(|state, tx| {
        state.body = TextEdit::new("An edit made while encryption is running");
        state.edit_changed(tx);
        state.handle(Event::Saved(4), tx);
    });
    let open = runtime.state.open().unwrap();
    assert_eq!(open.revision, 5);
    assert!(matches!(open.save, SavePhase::Waiting { .. }));
}

#[test]
fn unknown_editor_and_protected_note_cannot_be_rewritten() {
    let mut item = Item::new_note("Super", "{\"root\":{\"children\":[1,2,3]}}");
    item.content["noteType"] = serde_json::json!("super");
    assert!(!editable(&item));
    let original = item.content.clone();
    assert!(item.set_text("plain").is_err());
    assert_eq!(item.content, original);
    item.content["noteType"] = serde_json::json!("plain-text");
    item.set_flag("protected", true).unwrap();
    assert!(!editable(&item));
}

#[test]
fn tags_and_search_filter_without_losing_hidden_notes() {
    let mut runtime = runtime();
    let count = runtime.state.open().unwrap().data.items.len();
    runtime.update(|state, tx| {
        state.tags = TextEdit::new("Engenharia, Projeto");
        state.apply_tags(tx);
        state.filter(Filter::Tag("Engenharia".into()));
    });
    assert_eq!(runtime.state.open().unwrap().visible.len(), 1);
    runtime.update(|state, _| {
        state.search = TextEdit::new("inexistente123");
        state.refresh();
    });
    assert_eq!(runtime.state.open().unwrap().visible.len(), 0);
    assert!(runtime.state.open().unwrap().data.items.len() >= count);
}

#[test]
fn closing_before_debounce_flushes_and_reopens_the_last_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.snrvault");
    let state = AppState::at(path.clone(), false);
    state
        .worker
        .send(Command::Open {
            path: path.clone(),
            password: Zeroizing::new("test-password-123".into()),
            create: true,
        })
        .unwrap();
    let event = wait_event(&state);
    let mut runtime = Runtime::new(
        state,
        crate::views::view,
        (1200.0, 800.0),
        Env::new(true, 1.0),
    );
    runtime.update(|state, tx| {
        state.handle(event, tx);
        state.new_note(tx);
        state.title = TextEdit::new("Saved on close");
        state.body = TextEdit::new("the very last keystroke");
        state.edit_changed(tx);
    });
    assert!(matches!(
        runtime.state.open().unwrap().save,
        SavePhase::Waiting { .. }
    ));
    runtime.state.flush_for_exit().unwrap();
    runtime.state.worker.send(Command::Lock).unwrap();
    assert!(matches!(wait_event(&runtime.state), Event::Locked));
    let (_vault, data) = Vault::open(&path, "test-password-123").unwrap();
    assert_eq!(data.items[0].text(), "the very last keystroke");
    assert_eq!(data.items[0].title(), "Saved on close");
}

#[test]
fn demo_does_not_create_a_vault_and_lock_clears_visible_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("must-not-exist.snrvault");
    let mut state = AppState::at(path.clone(), true);
    state.demo_scene();
    assert!(!path.exists());
    state.clear_locked();
    assert!(matches!(state.session, Session::Locked));
    assert!(
        state.title.text.is_empty() && state.body.text.is_empty() && state.search.text.is_empty()
    );
    assert!(!path.exists());
}

fn reveal(runtime: &mut Runtime<AppState>, key: &'static str) {
    runtime.update(move |_, tx| tx.scroll_into_view(key));
    for _ in 0..40 {
        runtime.frame(1.0 / 60.0);
    }
}

#[test]
fn account_and_backup_panels_remain_reachable_at_minimum_window_size() {
    let mut runtime = runtime();
    // The fixture has only in-memory notes; opening a panel performs no network request.
    runtime.update(|state, _| state.demo = false);
    runtime.set_viewport((540.0, 520.0), 1.0);
    runtime.frame(0.0);
    tap(&mut runtime, "account");
    assert!(runtime.state.account_ui.visible);
    assert!(runtime.find("account-email/value/input").is_some());
    assert!(runtime.find("account-password/value/input").is_some());
    reveal(&mut runtime, "account-encrypted-export");
    let (x, y) = runtime.find("account-encrypted-export").unwrap();
    assert!((0.0..540.0).contains(&x) && (0.0..520.0).contains(&y));
    tap(&mut runtime, "account-encrypted-export");
    assert!(matches!(
        runtime.state.backup_ui.mode,
        Some(crate::backup_ui::Mode::Export)
    ));
    assert!(runtime.find("backup-password/value/input").is_some());
    tap(&mut runtime, "backup-cancel");
    assert!(runtime.state.backup_ui.mode.is_none());
    assert!(runtime.state.account_ui.visible);
    reveal(&mut runtime, "account-back");
    tap(&mut runtime, "account-back");
    assert!(!runtime.state.account_ui.visible);
    assert!(runtime.find("note-title/value/input").is_some());
}

#[test]
fn minimum_window_keeps_note_statistics_above_the_status_bar() {
    let mut runtime = runtime();
    runtime.set_viewport((540.0, 520.0), 1.0);
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
    let elements = runtime.elements();
    let statistics = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("note-stats"))
        .unwrap();
    let status = elements
        .iter()
        .find(|e| e.text.as_deref() == Some(runtime.state.status.as_str()))
        .unwrap();
    let footer = &elements[status.parent.unwrap()];
    assert!(
        statistics.rect[1] + statistics.rect[3] <= footer.rect[1],
        "statistics {:?} covered by footer {:?}",
        statistics.rect,
        footer.rect
    );
}

#[test]
fn busy_network_blocks_shortcuts_typing_and_note_mutations() {
    let mut runtime = runtime();
    let before = runtime.state.open().unwrap().data.clone();
    tap(&mut runtime, "note-body/scroll/input");
    runtime.update(|state, _| {
        state.demo = false;
        state.account_ui.phase = crate::account::Phase::Sync;
    });
    runtime.frame(0.0);
    assert!(runtime.state.busy());
    assert!(!runtime.state.can_edit());
    runtime.key(KeyEvent {
        key: Key::Command('n'),
        mods: Mods {
            cmd: true,
            ctrl: true,
            ..Default::default()
        },
    });
    runtime.key(KeyEvent {
        key: Key::Text("must not be inserted".into()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    tap(&mut runtime, "toggle-trash");
    runtime.update(|state, tx| {
        state.new_note(tx);
        state.toggle_flag("archived", tx);
        state.edit_changed(tx);
    });
    assert!(runtime.state.open().unwrap().data == before);
    assert!(
        runtime.find("note-body/scroll/input").is_none(),
        "busy editors are not mounted"
    );
}

#[test]
fn encrypted_backup_worker_detects_password_and_preserves_data_and_metadata() {
    use serde_json::json;
    use sn_rune_core::{BackupDocument, VaultData, set_note_tags};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("encrypted-backup.sn");
    let mut source = VaultData::default();
    let mut note = Item::new_note("Backup sintético", "Texto original: ação e informação.");
    note.content["custom_field"] = json!({"unrecognized": [1, 2, "preserve"]});
    let note_id = note.uuid.clone();
    source.items.push(note);
    let mut super_note = Item::new_note(
        "Documento opaco",
        "{\"root\":{\"children\":[{\"type\":\"table\"}]}}",
    );
    super_note.content["noteType"] = json!("super");
    source.items.push(super_note);
    set_note_tags(&mut source, &note_id, &["Fixture".into()]).unwrap();
    let document = BackupDocument {
        items: source.items.clone(),
        metadata: std::collections::BTreeMap::from([(
            "custom_envelope".into(),
            json!({"nested": ["retained", 42]}),
        )]),
    };
    let mut runtime = runtime();
    runtime.state.demo = false;
    let initial = runtime.state.open().unwrap().data.clone();
    runtime
        .state
        .worker
        .send(Command::ExportEncrypted {
            path: path.clone(),
            document: document.clone(),
            password: Zeroizing::new("backup-password-123".into()),
        })
        .unwrap();
    assert!(matches!(
        wait_event(&runtime.state),
        Event::Exported { encrypted: true }
    ));
    let bytes = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("Texto original"));
    runtime
        .state
        .worker
        .send(Command::Import { path: path.clone() })
        .unwrap();
    let event = wait_event(&runtime.state);
    assert!(matches!(&event, Event::EncryptedImportNeeded(found) if found == &path));
    runtime.update(|state, tx| state.handle(event, tx));
    assert!(
        matches!(&runtime.state.backup_ui.mode, Some(crate::backup_ui::Mode::Import(found)) if found == &path)
    );
    assert!(runtime.state.open().unwrap().data == initial);
    runtime
        .state
        .worker
        .send(Command::ImportEncrypted {
            path: path.clone(),
            password: Zeroizing::new("wrong-password".into()),
        })
        .unwrap();
    let event = wait_event(&runtime.state);
    assert!(matches!(
        event,
        Event::Failed {
            operation: "import",
            ..
        }
    ));
    runtime.update(|state, tx| state.handle(event, tx));
    assert!(
        runtime.state.open().unwrap().data == initial,
        "bad passwords cannot partially merge a backup"
    );
    assert!(
        runtime.state.backup_ui.mode.is_some(),
        "the password prompt stays open for a retry"
    );
    runtime
        .state
        .worker
        .send(Command::ImportEncrypted {
            path: path.clone(),
            password: Zeroizing::new("backup-password-123".into()),
        })
        .unwrap();
    let event = wait_event(&runtime.state);
    assert!(matches!(event, Event::Imported(_)));
    runtime.update(|state, tx| state.handle(event, tx));
    let imported = &runtime.state.open().unwrap().data;
    for original in &source.items {
        let saved = imported
            .items
            .iter()
            .find(|saved| saved.uuid == original.uuid)
            .expect("every item survives");
        assert_eq!(saved.content, original.content);
    }
    assert_eq!(
        imported.backup_metadata["custom_envelope"],
        document.metadata["custom_envelope"]
    );
    assert!(runtime.state.backup_ui.mode.is_none());
    // Existing backups are never truncated, even when the second export uses another password.
    runtime
        .state
        .worker
        .send(Command::ExportEncrypted {
            path: path.clone(),
            document,
            password: Zeroizing::new("another-password".into()),
        })
        .unwrap();
    assert!(matches!(
        wait_event(&runtime.state),
        Event::Failed {
            operation: "export",
            ..
        }
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn save_ack_queued_before_a_successful_flush_cannot_revert_clean_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flush.snrvault");
    let state = AppState::at(path.clone(), false);
    state
        .worker
        .send(Command::Open {
            path: path.clone(),
            password: Zeroizing::new("flush-password-123".into()),
            create: true,
        })
        .unwrap();
    let event = wait_event(&state);
    let mut runtime = Runtime::new(
        state,
        crate::views::view,
        (1200.0, 800.0),
        Env::new(true, 1.0),
    );
    runtime.update(|state, tx| {
        state.handle(event, tx);
        state.new_note(tx);
        state.body = TextEdit::new("revision before save");
        state.edit_changed(tx);
        state.save_now();
        state.body = TextEdit::new("final revision saved by flush");
        state.edit_changed(tx);
    });
    let final_revision = runtime.state.open().unwrap().revision;
    runtime.state.flush_for_exit().unwrap();
    assert!(matches!(
        runtime.state.open().unwrap().save,
        SavePhase::Clean
    ));
    let event = wait_event(&runtime.state);
    assert!(matches!(event, Event::Saved(revision) if revision < final_revision));
    runtime.update(|state, tx| state.handle(event, tx));
    assert!(matches!(
        runtime.state.open().unwrap().save,
        SavePhase::Clean
    ));
    assert_eq!(
        runtime.state.selected().unwrap().text(),
        "final revision saved by flush"
    );
    runtime.state.worker.send(Command::Lock).unwrap();
    assert!(matches!(wait_event(&runtime.state), Event::Locked));
    let (_, saved) = Vault::open(&path, "flush-password-123").unwrap();
    assert_eq!(saved.items[0].text(), "final revision saved by flush");
}

#[test]
fn closing_defers_until_network_or_backup_work_has_finished() {
    for network in [true, false] {
        let mut runtime = runtime();
        runtime.update(|state, tx| {
            if network {
                state.account_ui.phase = crate::account::Phase::Sync;
            } else {
                state.io = crate::state::IoPhase::Importing;
            }
            crate::closing(state, tx);
        });
        assert!(runtime.state.close_requested);
        assert!(
            runtime.state.open().is_some(),
            "documents remain alive while work can return credentials or imported items"
        );
        assert!(runtime.state.status.contains("antes de fechar"));
        runtime.update(|state, tx| {
            state.account_ui.phase = crate::account::Phase::Idle;
            state.io = crate::state::IoPhase::Idle;
            crate::state::poll(state, 0.0, tx);
        });
        assert!(
            !runtime.state.close_requested,
            "the deferred close is completed after the operation settles"
        );
    }
}

#[test]
fn pending_file_dialog_blocks_unlock_and_ignores_stale_completion() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("must-not-open.snrvault");
    let mut state = AppState::at(path.clone(), false);
    state.password = TextEdit::new("valid-fixture-password").secure();
    state.confirmation = TextEdit::new("valid-fixture-password").secure();
    let mut locked = Runtime::new(
        state,
        crate::views::view,
        (540.0, 520.0),
        Env::new(true, 1.0),
    );
    locked.frame(0.0);
    locked.update(|state, tx| {
        state.choose_vault(false, tx);
        state.unlock();
    });
    assert!(matches!(locked.state.session, Session::Locked));
    assert!(locked.state.busy());
    assert_eq!(locked.file_dialog().unwrap().0, rune::FileDialogKind::Open);
    assert!(!path.exists());
    assert!(locked.state.worker.try_recv().is_none());
    let next_path = directory.path().join("selected-vault.snrvault");
    locked.resolve_file_dialog(Some(next_path.clone())).unwrap();
    locked.frame(1.0 / 60.0);
    assert_eq!(locked.state.path, next_path);
    assert!(!locked.state.busy());
    assert!(locked.file_dialog().is_none());
    assert!(
        !next_path.exists(),
        "selecting a path does not create a vault"
    );

    let state = &mut locked.state;
    let first_generation = state.begin_dialog();
    let second_generation = state.begin_dialog();
    assert!(!state.finish_dialog(first_generation));
    assert!(
        state.busy(),
        "a stale callback cannot release a newer dialog"
    );
    assert!(state.finish_dialog(second_generation));
    assert!(!state.busy());
    assert!(!state.finish_dialog(second_generation));
    assert!(!path.exists());

    let mut runtime = runtime();
    let before = runtime.state.open().unwrap().data.clone();
    runtime.update(|state, tx| {
        state.begin_dialog();
        state.new_note(tx);
        state.toggle_flag("trashed", tx);
        state.request_lock(tx);
        crate::closing(state, tx);
    });
    assert!(runtime.state.open().is_some());
    assert!(runtime.state.open().unwrap().data == before);
    assert!(runtime.state.close_requested);
}

#[test]
fn cancelling_import_and_export_dialogs_preserves_the_open_vault() {
    for export in [false, true] {
        let mut runtime = runtime();
        runtime.state.demo = false;
        let before = runtime.state.open().unwrap().data.clone();
        runtime.update(|state, tx| {
            if export {
                state.export_dialog(tx);
            } else {
                state.import_dialog(tx);
            }
            state.request_lock(tx);
        });
        assert!(runtime.state.busy());
        assert!(matches!(runtime.state.session, Session::Open(_)));
        assert_eq!(
            runtime.file_dialog().unwrap().0,
            if export {
                rune::FileDialogKind::Save
            } else {
                rune::FileDialogKind::Open
            }
        );
        runtime.resolve_file_dialog(None).unwrap();
        runtime.frame(1.0 / 60.0);
        assert!(runtime.state.io == crate::state::IoPhase::Idle);
        assert!(matches!(runtime.state.session, Session::Open(_)));
        assert!(runtime.state.open().unwrap().data == before);
        assert!(runtime.state.worker.try_recv().is_none());
    }
}

#[test]
fn a_background_save_failure_does_not_cancel_an_unrelated_file_dialog() {
    let mut runtime = runtime();
    let mut generation = 0;
    runtime.update(|state, tx| {
        generation = state.begin_dialog();
        state.handle(
            Event::Failed {
                operation: "save",
                message: "Synthetic disk failure".into(),
            },
            tx,
        );
    });
    assert!(runtime.state.io == crate::state::IoPhase::Choosing);
    assert!(runtime.state.finish_dialog(generation));
    assert!(matches!(
        runtime.state.open().unwrap().save,
        SavePhase::Failed
    ));
}

/// Deliberately opt-in: uses the real offscreen GPU renderer, synthetic documents and no
/// account/network actions. It is not part of the normal unit test runtime.
#[test]
#[ignore = "visual validation requires a GPU/software Vulkan driver and an output directory"]
fn capture_desktop_and_account_fixtures() {
    let output = std::env::var_os("SN_RUNE_SCREENSHOT_DIR")
        .map(std::path::PathBuf::from)
        .expect("set SN_RUNE_SCREENSHOT_DIR to a scratch output directory");
    std::fs::create_dir_all(&output).unwrap();
    for (name, size, panel) in [
        ("final-wide", "1200x800", 0),
        ("final-compact", "540x520", 0),
        ("workspace-compact", "540x520", 3),
        ("account-compact", "540x520", 1),
        ("backup-compact", "540x520", 2),
    ] {
        let spec = rune::AppSpec::new(
            "Synthetic UI validation",
            (1200.0, 800.0),
            AppState::new,
            crate::views::view,
        );
        let app = rune::App::new(spec)
            .init(move || {
                let mut state = AppState::at("never-created-fixture.snrvault".into(), true);
                state.demo_scene();
                if panel != 0 {
                    state.demo = false;
                    state.account_ui.visible = panel == 1;
                    if panel == 2 {
                        state.backup_ui.mode = Some(crate::backup_ui::Mode::Export);
                    }
                }
                state
            })
            .args([
                "--size".to_owned(),
                size.to_owned(),
                "--at".to_owned(),
                "0.0".to_owned(),
                "--snap".to_owned(),
                output
                    .join(format!("{name}.png"))
                    .to_string_lossy()
                    .into_owned(),
                "--tree".to_owned(),
                output
                    .join(format!("{name}.json"))
                    .to_string_lossy()
                    .into_owned(),
            ]);
        app.try_run().unwrap();
    }
}
