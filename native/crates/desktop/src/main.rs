#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod account;
mod backup_ui;
mod editor;
mod state;
#[cfg(test)]
mod tests;
mod views;
mod worker;

use rune::{App, AppSpec, KeyEvent, Tx};
use state::{AppState, SavePhase};

fn shortcuts(s: &mut AppState, key: &KeyEvent, tx: &mut Tx) -> bool {
    if s.busy() || s.account_ui.visible || s.backup_ui.mode.is_some() {
        return false;
    }
    if key.is_command('n') && s.open().is_some() {
        s.new_note(tx);
        return true;
    }
    if key.is_command('s') && s.open().is_some() {
        s.apply_tags(tx);
        s.save_now();
        return true;
    }
    if key.is_command('l') && s.open().is_some() {
        s.request_lock(tx);
        return true;
    }
    if key.is_command('f') && s.open().is_some() {
        s.compact_editor = false;
        tx.focus("search", true);
        return true;
    }
    false
}

fn closing(s: &mut AppState, tx: &mut Tx) {
    tx.clear_secret_clipboard();
    if s.busy() || s.io != state::IoPhase::Idle {
        s.close_requested = true;
        s.status = "Concluindo a operação e salvando antes de fechar…".into();
        if let Some(handle) = &s.handle {
            handle.open();
        }
        return;
    }
    s.close_requested = false;
    s.apply_tags(tx);
    match s.flush_for_exit() {
        Ok(()) => tx.quit(),
        Err(error) => {
            s.error = Some(format!(
                "Não foi possível fechar com segurança: {error} Suas alterações continuam abertas."
            ));
            s.status = "Não salvo · corrija o erro e tente salvar novamente".into();
            if let Some(open) = s.open_mut() {
                open.save = SavePhase::Failed;
            }
            // keep_running retains the document if saving fails. Reopen after Rune hides it.
            if let Some(handle) = &s.handle {
                handle.open();
            }
        }
    }
}

fn quitting(s: &mut AppState, tx: &mut Tx) {
    tx.clear_secret_clipboard();
    s.apply_tags(tx);
    if let Err(error) = s.flush_for_exit() {
        // A forced OS shutdown cannot be vetoed by Rune. Never log note contents or keys.
        eprintln!("Falha no salvamento final do cofre: {error}");
    }
}

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("standardnotes-rune {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    rune::set_app_id("io.github.vitorhubdev.StandardNotesRune");
    let spec = AppSpec {
        min_size: Some((540.0, 520.0)),
        setup: Some(|s: &mut AppState, wake| {
            s.account_ui.service.set_waker(wake.clone());
            s.worker.set_waker(wake);
        }),
        poll: Some(state::poll),
        on_key: Some(shortcuts),
        scenes: &[("demo", AppState::demo_scene)],
        ..AppSpec::new(
            "Standard Notes Rune",
            (1200.0, 800.0),
            AppState::new,
            views::view,
        )
    };
    let app = App::new(spec)
        .keep_running()
        .on_close(closing)
        .on_quit(quitting);
    let handle = app.handle();
    let app = app.init(move || {
        let mut state = AppState::new();
        state.handle = Some(handle);
        state
    });
    if let Err(error) = app.try_run() {
        eprintln!("Não foi possível iniciar Standard Notes Rune: {error}");
        std::process::exit(1);
    }
}
