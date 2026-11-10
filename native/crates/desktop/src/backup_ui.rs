use std::path::PathBuf;

use rune::{El, FileDialogOptions, ScrollSpec, TextEdit, TextStyle, Tx, col, row, text};
use rune_kit::{Kit, TextInput};
use sn_rune_core::BackupDocument;
use zeroize::Zeroizing;

use crate::state::{AppState, IoPhase};
use crate::views::{button, label};
use crate::worker::Command;

#[derive(Clone)]
pub enum Mode {
    Import(PathBuf),
    Export,
}

pub struct Panel {
    pub mode: Option<Mode>,
    pub password: TextEdit,
    pub confirmation: TextEdit,
}

impl Panel {
    pub fn new() -> Self {
        Self {
            mode: None,
            password: TextEdit::new("").secure(),
            confirmation: TextEdit::new("").secure(),
        }
    }

    pub fn clear(&mut self) {
        self.mode = None;
        self.password.clear();
        self.confirmation.clear();
    }

    pub fn import(&mut self, path: PathBuf) {
        self.clear();
        self.mode = Some(Mode::Import(path));
    }
}

pub fn open_export(s: &mut AppState, tx: &mut Tx) {
    if s.demo || s.busy() || s.io != IoPhase::Idle {
        return;
    }
    s.apply_tags(tx);
    s.backup_ui.clear();
    s.backup_ui.mode = Some(Mode::Export);
    s.error = None;
}

fn submit(s: &mut AppState, tx: &mut Tx) {
    if s.io != IoPhase::Idle {
        return;
    }
    let Some(mode) = s.backup_ui.mode.clone() else {
        return;
    };
    if matches!(mode, Mode::Export) {
        if s.backup_ui.password.text.chars().count() < 8 {
            s.error = Some("Use pelo menos 8 caracteres para a senha do backup.".into());
            return;
        }
        if s.backup_ui.password.text != s.backup_ui.confirmation.text {
            s.error = Some("As senhas do backup não coincidem.".into());
            return;
        }
    }
    let password = Zeroizing::new(s.backup_ui.password.text.clone());
    s.backup_ui.password.clear();
    s.backup_ui.confirmation.clear();
    s.error = None;
    match mode {
        Mode::Import(path) => {
            if s.send(Command::ImportEncrypted { path, password }) {
                s.io = IoPhase::Importing;
                s.status = "Decifrando e verificando o backup…".into();
            }
        }
        Mode::Export => {
            let options = FileDialogOptions {
                title: "Exportar backup criptografado".into(),
                ..Default::default()
            }
            .filter("Backup Standard Notes", &["sn"]);
            let generation = s.begin_dialog();
            let original_path = s.path.clone();
            tx.save_file(options, move |s: &mut AppState, result, _| {
                if !s.finish_dialog(generation) || s.path != original_path || s.open().is_none() {
                    return;
                }
                match result {
                    Ok(Some(path)) => {
                        if let Some(open) = s.open() {
                            let document = BackupDocument {
                                items: open.data.items.clone(),
                                metadata: open.data.backup_metadata.clone(),
                            };
                            if s.send(Command::ExportEncrypted {
                                path,
                                document,
                                password: password.clone(),
                            }) {
                                s.io = IoPhase::Exporting;
                                s.status = "Criptografando backup…".into();
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => s.error = Some(error.to_string()),
                }
            });
        }
    }
}

pub fn view(s: &AppState, k: &Kit, height: f32) -> El {
    let import = matches!(s.backup_ui.mode, Some(Mode::Import(_)));
    let running = s.io != IoPhase::Idle;
    let title = if import {
        "Importar backup criptografado"
    } else {
        "Exportar backup criptografado"
    };
    let mut form = col().w_full().max_w(640.0).gap(14.0).padding(24.0)
        .child(row().gap(12.0).w_full().center_y()
            .child(text(title).font(TextStyle::new(23.0, 600.0)).color(k.c.text_base).wrap().grow(1.0))
            .child(button(k, "backup-cancel", "Voltar", !running, |s, _| s.backup_ui.clear())))
        .child(label(if import {
            "Informe a senha usada no backup Standard Notes. A importação verifica todos os itens antes de alterar o cofre. Protocolos antigos ou formatos não suportados geram um erro e preservam o arquivo."
        } else {
            "O arquivo .sn usa o protocolo 004 do Standard Notes. Escolha uma senha para este backup e guarde-a; sem ela, o arquivo não poderá ser aberto. Credenciais da conta não são incluídas."
        }, 14.0, k.c.text_muted).wrap());
    if let Some(Mode::Import(path)) = &s.backup_ui.mode {
        form = form.child(label(path.to_string_lossy(), 12.0, k.c.text_muted).truncate_middle());
    }
    form = form
        .child(label("Senha do backup", 13.0, k.c.text_base))
        .child(
            TextInput::new(&s.backup_ui.password)
                .width(None)
                .secure()
                .label("Senha do backup")
                .disabled(running)
                .on_edit(|s: &mut AppState| &mut s.backup_ui.password)
                .build(k)
                .key("backup-password"),
        );
    if !import {
        form = form
            .child(label("Confirme a senha", 13.0, k.c.text_base))
            .child(
                TextInput::new(&s.backup_ui.confirmation)
                    .width(None)
                    .secure()
                    .label("Confirme a senha do backup")
                    .disabled(running)
                    .on_edit(|s: &mut AppState| &mut s.backup_ui.confirmation)
                    .build(k)
                    .key("backup-password-confirm"),
            );
    }
    form = form.child(button(
        k,
        "backup-submit",
        if running {
            "Aguarde…"
        } else if import {
            "Decifrar e importar"
        } else {
            "Escolher destino e exportar"
        },
        !running,
        submit,
    ));
    if let Some(error) = &s.error {
        form = form.child(label(error, 13.0, k.c.danger.fg).wrap().role("alert"));
    }
    if running {
        form = form.child(label(&s.status, 13.0, k.c.text_muted).wrap().role("status"));
    }
    col()
        .w_full()
        .h(height)
        .items_center()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(form)
}
