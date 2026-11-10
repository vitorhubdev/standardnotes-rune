//! Network work has its own thread, so a stalled request never queues ahead of a
//! vault flush. The interface freezes edits during a sync and applies a complete
//! result only when its originating document revision still matches.
use std::sync::{Arc, Mutex, mpsc};

use rune::{El, ScrollSpec, TextEdit, TextStyle, Tx, Waker, col, el, row, text};
use rune_kit::{Kit, TextInput};
use serde_json::{Value, json};
use sn_rune_core::VaultData;
use sn_rune_protocol::AccountSession;
use zeroize::Zeroizing;

use crate::state::{AppState, IoPhase, SavePhase};
use crate::views::{button, label};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    PreparingLogin,
    PreparingSync,
    Login,
    Sync,
}

pub struct Panel {
    pub visible: bool,
    pub phase: Phase,
    pub server: TextEdit,
    pub email: TextEdit,
    pub password: TextEdit,
    pub mfa: TextEdit,
    pub message: String,
    pub confirm_disconnect: bool,
    pub service: Service,
    pending: Option<Request>,
}

impl Panel {
    pub fn new() -> Self {
        Self {
            visible: false,
            phase: Phase::Idle,
            server: TextEdit::new("https://api.standardnotes.com"),
            email: TextEdit::new(""),
            password: TextEdit::new("").secure(),
            mfa: TextEdit::new("").secure(),
            message: String::new(),
            confirm_disconnect: false,
            service: Service::new(),
            pending: None,
        }
    }

    pub fn running(&self) -> bool {
        self.phase != Phase::Idle
    }

    pub fn clear(&mut self) {
        self.visible = false;
        self.password.clear();
        self.mfa.clear();
        self.email.clear();
        self.message.clear();
        self.confirm_disconnect = false;
        self.pending = None;
    }
}

enum Action {
    Login {
        server: String,
        email: String,
        password: Zeroizing<String>,
        mfa: Zeroizing<String>,
    },
    Sync,
}

struct Request {
    revision: u64,
    data: VaultData,
    action: Action,
}

struct Response {
    revision: u64,
    data: VaultData,
    result: Result<String, String>,
}

pub struct Service {
    requests: mpsc::Sender<Request>,
    responses: mpsc::Receiver<Response>,
    wake: Arc<Mutex<Option<Waker>>>,
}

impl Service {
    fn new() -> Self {
        let (requests, incoming) = mpsc::channel::<Request>();
        let (answers, responses) = mpsc::channel();
        let wake: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let wake_thread = wake.clone();
        std::thread::Builder::new()
            .name("notes-network".into())
            .spawn(move || {
                while let Ok(mut request) = incoming.recv() {
                    let result = perform(&mut request.data, request.action)
                        .map_err(|error| error.to_string());
                    if answers
                        .send(Response {
                            revision: request.revision,
                            data: request.data,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                    if let Ok(wake) = wake_thread.lock()
                        && let Some(wake) = wake.as_ref()
                    {
                        wake();
                    }
                }
            })
            .expect("could not start the account worker");
        Self {
            requests,
            responses,
            wake,
        }
    }

    pub fn set_waker(&self, wake: Waker) {
        if let Ok(mut current) = self.wake.lock() {
            *current = Some(wake);
        }
    }
}

fn perform(data: &mut VaultData, action: Action) -> anyhow::Result<String> {
    match action {
        Action::Login {
            server,
            email,
            password,
            mfa,
        } => {
            let session = sn_rune_protocol::login(&server, &email, &password, Some(&mfa))?;
            if let Some(bound) = data.settings.get("bound_account") {
                anyhow::ensure!(
                    bound.get("user_uuid").and_then(Value::as_str)
                        == Some(session.user_uuid.as_str())
                        && bound.get("server").and_then(Value::as_str)
                            == Some(session.server.as_str()),
                    "Este cofre pertence a outra conta ou servidor. Crie outro cofre para manter os dados separados."
                );
            }
            data.settings.insert("bound_account".into(), json!({
                "user_uuid": session.user_uuid, "server": session.server, "email": session.email,
            }));
            data.account = Some(session.to_vault_value()?);
            Ok(
                "Conta conectada. Use Sincronizar para buscar alterações e enviar notas locais."
                    .into(),
            )
        }
        Action::Sync => {
            let value = data
                .account
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Entre na conta antes de sincronizar."))?;
            let mut session = AccountSession::from_vault_value(value)?;
            let result = sn_rune_protocol::sync(&mut session, data);
            // A refresh can rotate the token even if a later request fails.
            // Always return the updated session to the encrypted persistence path.
            data.account = Some(session.to_vault_value()?);
            let report = result?;
            Ok(format!(
                "Sincronização concluída: {} recebidos, {} enviados, {} conflitos preservados, {} pendentes.",
                report.downloaded, report.uploaded, report.conflicts, report.pending
            ))
        }
    }
}

pub fn open(s: &mut AppState, tx: &mut Tx) {
    if s.demo || s.busy() || s.io != IoPhase::Idle {
        return;
    }
    s.apply_tags(tx);
    let account = s
        .open()
        .and_then(|open| open.data.account.as_ref())
        .cloned();
    if let Some(value) = account {
        if let Some(server) = value.get("server").and_then(Value::as_str) {
            s.account_ui.server = TextEdit::new(server);
        }
        if let Some(email) = value.get("email").and_then(Value::as_str) {
            s.account_ui.email = TextEdit::new(email);
        }
    }
    s.account_ui.password.clear();
    s.account_ui.mfa.clear();
    s.account_ui.visible = true;
    s.account_ui.confirm_disconnect = false;
    s.error = None;
}

pub fn login(s: &mut AppState, tx: &mut Tx) {
    if s.demo || s.busy() || s.io != IoPhase::Idle {
        return;
    }
    if s.account_ui.password.text.is_empty() || s.account_ui.email.text.trim().is_empty() {
        s.error = Some("Informe e-mail e senha da conta Standard Notes.".into());
        return;
    }
    let action = Action::Login {
        server: s.account_ui.server.text.clone(),
        email: s.account_ui.email.text.clone(),
        password: Zeroizing::new(s.account_ui.password.text.clone()),
        mfa: Zeroizing::new(s.account_ui.mfa.text.clone()),
    };
    s.account_ui.password.clear();
    s.account_ui.mfa.clear();
    launch(s, action, Phase::Login, tx);
}

pub fn sync(s: &mut AppState, tx: &mut Tx) {
    if s.demo || s.busy() || s.io != IoPhase::Idle {
        return;
    }
    launch(s, Action::Sync, Phase::Sync, tx);
}

fn launch(s: &mut AppState, action: Action, phase: Phase, tx: &mut Tx) {
    s.apply_tags(tx);
    let Some(open) = s.open() else {
        return;
    };
    let request = Request {
        revision: open.revision,
        data: open.data.clone(),
        action,
    };
    // Wait asynchronously for the latest vault acknowledgement before any
    // network side effect. Disk failure stops the operation at this boundary.
    s.account_ui.pending = Some(request);
    s.account_ui.phase = if phase == Phase::Login {
        Phase::PreparingLogin
    } else {
        Phase::PreparingSync
    };
    s.account_ui.message = "Salvando o cofre antes de conectar…".into();
    s.error = None;
    if s.open()
        .is_some_and(|open| !matches!(open.save, SavePhase::Clean))
    {
        s.save_now();
    }
    tx.wake_after(std::time::Duration::ZERO);
}

pub fn disconnect(s: &mut AppState, tx: &mut Tx) {
    if s.busy() {
        return;
    }
    if let Some(open) = s.open_mut() {
        let mut secrets = VaultData {
            account: open.data.account.take(),
            ..Default::default()
        };
        secrets.clear_sensitive();
        s.account_ui.confirm_disconnect = false;
        s.account_ui.message = "Sessão removida deste cofre. Suas notas foram mantidas; a sessão no servidor pode ser revogada no aplicativo original.".into();
        s.changed(tx);
    }
}

/// A stale document response may still contain a rotated session or a successful
/// first login. Keep only that session and its matching account binding; never
/// replace local documents or a binding belonging to another account.
fn retain_concurrent_account(
    local: &mut VaultData,
    incoming: &mut VaultData,
) -> anyhow::Result<()> {
    let Some(account) = incoming.account.as_ref() else {
        return Ok(());
    };
    let session = AccountSession::from_vault_value(account)?;
    anyhow::ensure!(
        !session.user_uuid.is_empty(),
        "A sessão recebida não identifica a conta"
    );
    let matches_session = |binding: &Value| {
        binding.get("user_uuid").and_then(Value::as_str) == Some(session.user_uuid.as_str())
            && binding.get("server").and_then(Value::as_str) == Some(session.server.as_str())
    };
    for binding in [
        local.settings.get("bound_account"),
        incoming.settings.get("bound_account"),
    ]
    .into_iter()
    .flatten()
    {
        anyhow::ensure!(
            matches_session(binding),
            "O vínculo do cofre não corresponde à conta e ao servidor da sessão recebida"
        );
    }
    if let Some(current) = local.account.as_ref() {
        let current = AccountSession::from_vault_value(current)?;
        anyhow::ensure!(
            current.user_uuid == session.user_uuid && current.server == session.server,
            "A conta local mudou enquanto a operação estava em andamento"
        );
    }
    // All checks precede mutation. Keep an existing compatible binding intact,
    // including its unknown fields; a first login brings its validated binding.
    let binding = incoming
        .settings
        .get("bound_account")
        .cloned()
        .unwrap_or_else(|| {
            json!({"user_uuid": session.user_uuid, "server": session.server, "email": session.email})
        });
    let mut account = incoming.account.take().expect("account was checked");
    account["sync_token"] = Value::Null;
    local.settings.entry("bound_account").or_insert(binding);
    let mut previous = VaultData {
        account: local.account.replace(account),
        ..Default::default()
    };
    previous.clear_sensitive();
    Ok(())
}

pub fn poll(s: &mut AppState, tx: &mut Tx) -> bool {
    let mut changed = false;
    if s.account_ui.pending.is_some() {
        let phase = s.open().map(|open| open.save);
        match phase {
            Some(SavePhase::Clean) => {
                let request = s.account_ui.pending.take().expect("pending was checked");
                if s.account_ui.service.requests.send(request).is_ok() {
                    s.account_ui.phase = if s.account_ui.phase == Phase::PreparingLogin {
                        Phase::Login
                    } else {
                        Phase::Sync
                    };
                    s.account_ui.message = if s.account_ui.phase == Phase::Login {
                        "Conectando à conta…"
                    } else {
                        "Sincronizando… Suas notas continuam salvas localmente."
                    }
                    .into();
                } else {
                    s.account_ui.phase = Phase::Idle;
                    s.error = Some(
                        "O serviço de rede parou. Suas notas continuam salvas neste cofre.".into(),
                    );
                }
                changed = true;
            }
            Some(SavePhase::Failed) | None => {
                s.account_ui.pending = None;
                s.account_ui.phase = Phase::Idle;
                s.account_ui.message =
                    "Conexão cancelada: primeiro salve o cofre e tente novamente.".into();
                changed = true;
            }
            _ => {}
        }
    }
    let Ok(mut response) = s.account_ui.service.responses.try_recv() else {
        return changed;
    };
    s.account_ui.phase = Phase::Idle;
    let Some(open) = s.open_mut() else {
        response.data.clear_sensitive();
        return true;
    };
    if open.revision != response.revision {
        // Defensive handling for a programmatic edit while controls were frozen:
        // keep local edits, retain rotated credentials, force a full next pull.
        let retained = retain_concurrent_account(&mut open.data, &mut response.data);
        response.data.clear_sensitive();
        s.error = Some(match retained {
            Ok(()) => "Houve uma edição concorrente. Ela foi preservada; sincronize novamente para reconciliar a conta.".into(),
            Err(error) => format!("A sessão recebida não foi aplicada: {error}. Os dados locais foram preservados."),
        });
    } else {
        open.data.clear_sensitive();
        open.data = response.data;
        let selection = open.selected.clone();
        s.refresh();
        if let Some(id) = selection {
            s.select(&id);
        }
        match response.result {
            Ok(message) => s.account_ui.message = message,
            Err(error) => {
                s.account_ui.message =
                    "A operação não foi concluída. Os dados locais foram preservados.".into();
                s.error = Some(error);
            }
        }
    }
    s.changed(tx);
    s.save_now();
    true
}

pub fn view(s: &AppState, k: &Kit, height: f32) -> El {
    let running = s.account_ui.running();
    let connected = s.open().is_some_and(|open| open.data.account.is_some());
    let mut content = col().w_full().max_w(760.0).gap(16.0).padding(24.0)
        .child(row().w_full().gap(12.0).center_y()
            .child(text("Conta e sincronização").font(TextStyle::new(24.0, 600.0)).color(k.c.text_base).grow(1.0))
            .child(button(k, "account-back", "Voltar", !running, |s, _| { s.account_ui.visible = false; s.account_ui.password.clear(); s.account_ui.mfa.clear(); })))
        .child(label("Protocolo 004 · contas privadas. Cofres compartilhados, passkeys e formatos legados ainda precisam do aplicativo original.", 13.0, k.c.text_muted).wrap());
    if connected {
        content = content
            .child(label(&s.account_ui.email.text, 17.0, k.c.text_base))
            .child(label(&s.account_ui.server.text, 13.0, k.c.text_muted).wrap())
            .child(label("Sincronizar busca as alterações da conta e envia as notas deste cofre. Conflitos de notas são preservados como cópias. As edições ficam pausadas durante a operação.", 14.0, k.c.text_muted).wrap())
            .child(button(k, "sync-now", if running { "Sincronizando…" } else { "Sincronizar agora" }, !running, sync))
            .child(button(k, "disconnect-account", "Desconectar deste cofre", !running, |s, _| s.account_ui.confirm_disconnect = true));
        if s.account_ui.confirm_disconnect {
            content = content.child(label("Apenas a sessão local será removida. As notas permanecem neste cofre, vinculado à mesma conta para evitar misturar dados de contas diferentes.", 13.0, k.c.text_muted).wrap())
                .child(row().gap(10.0).flex_wrap()
                    .child(button(k, "confirm-disconnect", "Remover sessão local", !running, disconnect))
                    .child(button(k, "cancel-disconnect", "Cancelar", true, |s, _| s.account_ui.confirm_disconnect = false)));
        }
    } else {
        content = content
            .child(label("Servidor", 13.0, k.c.text_base))
            .child(
                TextInput::new(&s.account_ui.server)
                    .width(None)
                    .label("Servidor Standard Notes")
                    .disabled(running)
                    .on_edit(|s: &mut AppState| &mut s.account_ui.server)
                    .build(k)
                    .key("account-server"),
            )
            .child(label("E-mail", 13.0, k.c.text_base))
            .child(
                TextInput::new(&s.account_ui.email)
                    .width(None)
                    .label("E-mail da conta")
                    .disabled(running)
                    .on_edit(|s: &mut AppState| &mut s.account_ui.email)
                    .build(k)
                    .key("account-email"),
            )
            .child(label("Senha da conta Standard Notes", 13.0, k.c.text_base))
            .child(
                TextInput::new(&s.account_ui.password)
                    .width(None)
                    .secure()
                    .label("Senha da conta")
                    .disabled(running)
                    .on_edit(|s: &mut AppState| &mut s.account_ui.password)
                    .build(k)
                    .key("account-password"),
            )
            .child(label(
                "Código de duas etapas (se necessário)",
                13.0,
                k.c.text_base,
            ))
            .child(
                TextInput::new(&s.account_ui.mfa)
                    .width(None)
                    .secure()
                    .label("Código de duas etapas")
                    .disabled(running)
                    .on_edit(|s: &mut AppState| &mut s.account_ui.mfa)
                    .build(k)
                    .key("account-mfa"),
            )
            .child(button(
                k,
                "account-login",
                if running {
                    "Conectando…"
                } else {
                    "Conectar conta"
                },
                !running,
                login,
            ));
    }
    if !s.account_ui.message.is_empty() {
        content = content.child(
            label(&s.account_ui.message, 13.0, k.c.text_muted)
                .wrap()
                .role("status"),
        );
    }
    if let Some(error) = &s.error {
        content = content.child(label(error, 13.0, k.c.danger.fg).wrap().role("alert"));
    }
    content = content.child(el().h(12.0)).child(label("A conta e suas chaves são salvas somente dentro do cofre criptografado. A senha original não é armazenada.", 12.0, k.c.text_muted).wrap())
        .child(el().h(8.0))
        .child(text("Backups e importação").font(TextStyle::new(18.0, 600.0)).color(k.c.text_base))
        .child(row().gap(8.0).flex_wrap()
            .child(button(k, "account-import", "Importar arquivo", !running && s.io == IoPhase::Idle, |s, tx| s.import_dialog(tx)))
            .child(button(k, "account-encrypted-export", "Backup com senha", !running && s.io == IoPhase::Idle, crate::backup_ui::open_export))
            .child(button(k, "account-plain-export", "Exportar sem senha", !running && s.io == IoPhase::Idle, |s, _| { s.account_ui.visible = false; s.confirm_export = true; })))
        .child(label(&s.status, 12.0, k.c.text_muted).wrap());
    col()
        .w_full()
        .h(height)
        .items_center()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sn_rune_core::Item;

    const USER: &str = "e285ec74-48d2-4dfd-a118-ed9713570c6f";
    const SERVER: &str = "https://api.standardnotes.com";

    fn account(user: &str, server: &str, token: &str) -> Value {
        json!({
            "server": server, "email": "fixture@example.invalid", "user_uuid": user,
            "root": {
                "master_key": "00".repeat(32), "server_password": "11".repeat(32),
                "params": {"identifier": "fixture@example.invalid", "pw_nonce": "22".repeat(32), "version": "004"}
            },
            "tokens": {"access_token": token, "refresh_token": format!("refresh-{token}")},
            "sync_token": "download-cursor"
        })
    }

    fn binding(user: &str, server: &str) -> Value {
        json!({"user_uuid": user, "server": server, "email": "fixture@example.invalid"})
    }

    #[test]
    fn concurrent_first_login_retains_binding_and_leaves_local_edits_intact() {
        let mut local = VaultData {
            items: vec![Item::new_note("Edited locally", "latest local text")],
            ..Default::default()
        };
        local.settings.insert("dark".into(), json!(true));
        let original_items = local.items.clone();
        let mut incoming = VaultData {
            account: Some(account(USER, SERVER, "new-token")),
            ..Default::default()
        };
        let mut expected_binding = binding(USER, SERVER);
        expected_binding["unknown_metadata"] = json!({"retain": true});
        incoming
            .settings
            .insert("bound_account".into(), expected_binding.clone());
        incoming.settings.insert("dark".into(), json!(false));

        retain_concurrent_account(&mut local, &mut incoming).unwrap();

        assert_eq!(local.items, original_items);
        assert_eq!(local.settings["dark"], json!(true));
        assert_eq!(local.settings["bound_account"], expected_binding);
        let stored = local.account.as_ref().unwrap();
        assert_eq!(stored["tokens"]["access_token"], "new-token");
        assert!(stored["sync_token"].is_null());
        assert!(
            !AccountSession::from_vault_value(stored)
                .unwrap()
                .has_synced()
        );
        assert!(incoming.account.is_none());
    }

    #[test]
    fn concurrent_refresh_keeps_existing_binding_and_rotated_credentials() {
        let mut local = VaultData {
            account: Some(account(USER, SERVER, "old-token")),
            ..Default::default()
        };
        let mut original_binding = binding(USER, SERVER);
        original_binding["local_metadata"] = json!(42);
        local
            .settings
            .insert("bound_account".into(), original_binding.clone());
        let mut incoming = VaultData {
            account: Some(account(USER, SERVER, "rotated-token")),
            ..Default::default()
        };
        incoming
            .settings
            .insert("bound_account".into(), binding(USER, SERVER));

        retain_concurrent_account(&mut local, &mut incoming).unwrap();

        assert_eq!(local.settings["bound_account"], original_binding);
        let stored = local.account.as_ref().unwrap();
        assert_eq!(stored["tokens"]["refresh_token"], "refresh-rotated-token");
        assert!(stored["sync_token"].is_null());
    }

    #[test]
    fn conflicting_account_or_server_does_not_replace_binding_or_credentials() {
        for (user, server) in [
            ("58cae49f-19b7-4c6e-944d-abd486691ddc", SERVER),
            (USER, "https://other.example.invalid"),
        ] {
            let mut local = VaultData {
                account: Some(account(USER, SERVER, "local-token")),
                ..Default::default()
            };
            local
                .settings
                .insert("bound_account".into(), binding(USER, SERVER));
            let mut incoming = VaultData {
                account: Some(account(user, server, "unrelated-token")),
                ..Default::default()
            };
            incoming
                .settings
                .insert("bound_account".into(), binding(user, server));
            let local_before = local.clone();
            let incoming_before = incoming.clone();

            assert!(retain_concurrent_account(&mut local, &mut incoming).is_err());
            assert_eq!(local, local_before);
            assert_eq!(incoming, incoming_before);
        }
    }

    #[test]
    fn incoming_binding_must_match_its_authenticated_session() {
        let mut local = VaultData::default();
        let mut incoming = VaultData {
            account: Some(account(USER, SERVER, "new-token")),
            ..Default::default()
        };
        incoming.settings.insert(
            "bound_account".into(),
            binding(USER, "https://other.example.invalid"),
        );
        assert!(retain_concurrent_account(&mut local, &mut incoming).is_err());
        assert_eq!(local, VaultData::default());
        assert!(incoming.account.is_some());
    }

    #[test]
    fn failed_login_without_a_session_cannot_remove_current_credentials() {
        let mut local = VaultData {
            account: Some(account(USER, SERVER, "current-token")),
            ..Default::default()
        };
        let before = local.clone();
        retain_concurrent_account(&mut local, &mut VaultData::default()).unwrap();
        assert_eq!(local, before);
    }
}
