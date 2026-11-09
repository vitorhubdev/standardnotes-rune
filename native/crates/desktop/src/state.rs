use std::path::PathBuf;

use rune::{FileDialogOptions, TextEdit, Tx};
use serde_json::json;
use sn_rune_core::{BackupDocument, Item, VaultData, merge_document, set_note_tags, tag_names};
use zeroize::Zeroizing;

use crate::worker::{Command, Event, Worker};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    Notes,
    Pinned,
    Archived,
    Trash,
    Other,
    Tag(String),
}

impl Filter {
    pub fn label(&self) -> &str {
        match self {
            Self::Notes => "Todas as notas",
            Self::Pinned => "Fixadas",
            Self::Archived => "Arquivo",
            Self::Trash => "Lixeira",
            Self::Other => "Outros itens",
            Self::Tag(name) => name,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SavePhase {
    Clean,
    Waiting { due: f64 },
    Writing { revision: u64 },
    Failed,
}

pub struct OpenSession {
    pub data: VaultData,
    pub selected: Option<String>,
    pub visible: Vec<usize>,
    pub filter: Filter,
    pub revision: u64,
    pub save: SavePhase,
    pub preview: bool,
}

pub enum Session {
    Locked,
    Opening,
    Open(OpenSession),
    Locking(OpenSession),
}
#[derive(PartialEq, Eq)]
pub enum IoPhase {
    Idle,
    Choosing,
    Importing,
    Exporting,
}

pub struct AppState {
    pub dialog_generation: u64,
    pub close_requested: bool,
    pub account_ui: crate::account::Panel,
    pub backup_ui: crate::backup_ui::Panel,
    pub handle: Option<rune::AppHandle<AppState>>,
    pub session: Session,
    pub worker: Worker,
    pub path: PathBuf,
    pub create: bool,
    pub password: TextEdit,
    pub confirmation: TextEdit,
    pub search: TextEdit,
    pub title: TextEdit,
    pub body: TextEdit,
    pub tags: TextEdit,
    pub dark: bool,
    pub demo: bool,
    pub status: String,
    pub error: Option<String>,
    pub io: IoPhase,
    pub confirm_export: bool,
    pub compact_editor: bool,
}

impl AppState {
    pub fn new() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let demo = args
            .windows(2)
            .any(|args| args[0] == "--scene" && args[1] == "demo");
        let path = if demo {
            PathBuf::from("demo-in-memory")
        } else {
            std::env::var_os("SN_RUNE_VAULT")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    rune::dirs::AppDirs::new("dev.vitorhub.standardnotes-rune")
                        .map(|dirs| dirs.data().join("notes.snrvault"))
                        .unwrap_or_else(|| PathBuf::from("notes.snrvault"))
                })
        };
        Self::at(path, demo)
    }

    pub fn at(path: PathBuf, demo: bool) -> Self {
        Self {
            dialog_generation: 0,
            close_requested: false,
            account_ui: crate::account::Panel::new(),
            backup_ui: crate::backup_ui::Panel::new(),
            handle: None,
            create: !demo && !path.exists(),
            path,
            session: Session::Locked,
            worker: Worker::new(),
            password: TextEdit::new("").secure(),
            confirmation: TextEdit::new("").secure(),
            search: TextEdit::new(""),
            title: TextEdit::new(""),
            body: TextEdit::new(""),
            tags: TextEdit::new(""),
            dark: true,
            demo,
            status: "Cofre bloqueado".into(),
            error: None,
            io: IoPhase::Idle,
            confirm_export: false,
            compact_editor: false,
        }
    }

    pub fn demo_scene(&mut self) {
        self.demo = true;
        self.path = PathBuf::from("demo-in-memory");
        let mut data = VaultData::default();
        let mut welcome = Item::new_note(
            "Um lugar para pensar",
            "# Suas ideias, por perto\n\nEsta é uma demonstração isolada. Nenhum cofre real é aberto e nenhuma nota é gravada no disco.\n\nEscreva, pesquise e organize suas notas com etiquetas. Use Ctrl+N para uma nova nota e Ctrl+S para salvar.\n\n## Próximos passos\n\n- Crie um cofre com uma senha exclusiva\n- Importe um backup do Standard Notes\n- Organize as notas com etiquetas\n\nNotas de editores ainda não suportados são preservadas sem alterações.",
        );
        let _ = welcome.set_flag("pinned", true);
        let id = welcome.uuid.clone();
        data.items.push(welcome);
        data.items.push(Item::new_note("Ideias para esta semana", "Revisar os projetos em andamento.\nSeparar um tempo para leitura.\nAnotar o que merece atenção."));
        data.items.push(Item::new_note(
            "Caderno de referências",
            "Uma boa nota deixa o contexto claro e torna o próximo passo mais fácil.",
        ));
        let _ = set_note_tags(&mut data, &id, &["Pessoal".into(), "Comece aqui".into()]);
        self.open_data(data);
        self.status = "Demonstração · somente em memória".into();
    }

    pub fn open(&self) -> Option<&OpenSession> {
        match &self.session {
            Session::Open(open) | Session::Locking(open) => Some(open),
            _ => None,
        }
    }
    pub fn open_mut(&mut self) -> Option<&mut OpenSession> {
        match &mut self.session {
            Session::Open(open) => Some(open),
            _ => None,
        }
    }
    pub fn busy(&self) -> bool {
        matches!(self.session, Session::Opening | Session::Locking(_))
            || self.account_ui.running()
            || self.io == IoPhase::Choosing
    }
    pub fn selected(&self) -> Option<&Item> {
        let open = self.open()?;
        let id = open.selected.as_ref()?;
        open.data.items.iter().find(|item| &item.uuid == id)
    }
    pub fn can_edit(&self) -> bool {
        !self.busy() && self.selected().is_some_and(editable)
    }

    pub fn send(&mut self, command: Command) -> bool {
        match self.worker.send(command) {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    pub fn unlock(&mut self) {
        if !matches!(self.session, Session::Locked) || self.demo || self.busy() {
            return;
        }
        if self.password.text.is_empty() {
            self.error = Some("Digite a senha do cofre.".into());
            return;
        }
        if self.create && self.password.text.chars().count() < 8 {
            self.error = Some("Use uma senha com pelo menos 8 caracteres.".into());
            return;
        }
        if self.create && self.password.text != self.confirmation.text {
            self.error = Some("As senhas não coincidem.".into());
            return;
        }
        let password = Zeroizing::new(self.password.text.clone());
        self.password.clear();
        self.confirmation.clear();
        self.error = None;
        if self.send(Command::Open {
            path: self.path.clone(),
            password,
            create: self.create,
        }) {
            self.session = Session::Opening;
            self.status = if self.create {
                "Criando cofre…"
            } else {
                "Desbloqueando…"
            }
            .into();
        }
    }

    pub fn choose_vault(&mut self, create: bool, tx: &mut Tx) {
        if self.busy() || self.demo || !matches!(self.session, Session::Locked) {
            return;
        }
        let options = FileDialogOptions {
            title: if create { "Criar cofre" } else { "Abrir cofre" }.into(),
            ..Default::default()
        }
        .filter("Cofre Rune", &["snrvault"]);
        let generation = self.begin_dialog();
        let original_path = self.path.clone();
        let answer = move |s: &mut AppState,
                           result: Result<Option<PathBuf>, rune::FileDialogError>,
                           _: &mut Tx| {
            if !s.finish_dialog(generation) {
                return;
            }
            if !matches!(s.session, Session::Locked) || s.path != original_path {
                return;
            }
            match result {
                Ok(Some(path)) => {
                    s.path = path;
                    s.create = create;
                    s.error = None;
                }
                Ok(None) => {}
                Err(error) => s.error = Some(error.to_string()),
            }
        };
        if create {
            tx.save_file(options, answer);
        } else {
            tx.open_file(options, answer);
        }
    }

    pub fn begin_dialog(&mut self) -> u64 {
        self.dialog_generation = self.dialog_generation.wrapping_add(1);
        self.io = IoPhase::Choosing;
        self.dialog_generation
    }

    pub fn finish_dialog(&mut self, generation: u64) -> bool {
        if self.dialog_generation != generation || self.io != IoPhase::Choosing {
            return false;
        }
        self.io = IoPhase::Idle;
        true
    }

    pub fn open_data(&mut self, data: VaultData) {
        self.dark = data
            .settings
            .get("dark")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        self.session = Session::Open(OpenSession {
            data,
            selected: None,
            visible: Vec::new(),
            filter: Filter::Notes,
            revision: 0,
            save: SavePhase::Clean,
            preview: false,
        });
        self.search.clear();
        self.error = None;
        self.status = "Tudo salvo neste dispositivo".into();
        self.refresh();
        let first = self.open().and_then(|open| {
            open.visible
                .first()
                .map(|&i| open.data.items[i].uuid.clone())
        });
        if let Some(id) = first {
            self.select(&id);
        }
    }

    pub fn refresh(&mut self) {
        let query = self.search.text.trim().to_lowercase();
        let Some(open) = self.open_mut() else {
            return;
        };
        open.visible = open
            .data
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                if item.deleted {
                    return None;
                }
                let note = item.content_type == "Note";
                let yes = match &open.filter {
                    Filter::Notes => note && !item.flag("trashed") && !item.flag("archived"),
                    Filter::Pinned => note && item.flag("pinned") && !item.flag("trashed"),
                    Filter::Archived => note && item.flag("archived") && !item.flag("trashed"),
                    Filter::Trash => note && item.flag("trashed"),
                    Filter::Other => !note && !item.is_tag(),
                    Filter::Tag(name) => {
                        note && !item.flag("trashed")
                            && tag_names(&open.data, &item.uuid).contains(name)
                    }
                };
                (yes && (query.is_empty()
                    || item.title().to_lowercase().contains(&query)
                    || item.text().to_lowercase().contains(&query)))
                .then_some(i)
            })
            .collect();
        open.visible.sort_by(|&a, &b| {
            let (a, b) = (&open.data.items[a], &open.data.items[b]);
            b.flag("pinned")
                .cmp(&a.flag("pinned"))
                .then_with(|| b.modified_at().cmp(a.modified_at()))
                .then_with(|| a.uuid.cmp(&b.uuid))
        });
    }

    pub fn filter(&mut self, filter: Filter) {
        if let Some(open) = self.open_mut() {
            open.filter = filter;
        }
        self.refresh();
        self.compact_editor = false;
    }

    pub fn select(&mut self, id: &str) {
        let Some(open) = self.open_mut() else {
            return;
        };
        let Some(item) = open.data.items.iter().find(|item| item.uuid == id) else {
            return;
        };
        let title = item.title().to_owned();
        let body = item.text().to_owned();
        let tags = tag_names(&open.data, id).join(", ");
        open.selected = Some(id.to_owned());
        open.preview = false;
        self.title = TextEdit::new(title);
        self.body = TextEdit::new(body);
        self.tags = TextEdit::new(tags);
        self.compact_editor = true;
    }

    pub fn new_note(&mut self, tx: &mut Tx) {
        if self.busy() {
            return;
        }
        self.apply_tags(tx);
        let Some(open) = self.open_mut() else {
            return;
        };
        let note = Item::new_note("", "");
        let id = note.uuid.clone();
        open.data.items.push(note);
        open.filter = Filter::Notes;
        self.search.clear();
        self.select(&id);
        self.changed(tx);
        tx.focus("note-title", true);
    }

    pub fn edit_changed(&mut self, tx: &mut Tx) {
        if !self.can_edit() {
            return;
        }
        let (title, body) = (self.title.text.clone(), self.body.text.clone());
        let Some(open) = self.open_mut() else {
            return;
        };
        let Some(item) = open
            .data
            .items
            .iter_mut()
            .find(|item| Some(&item.uuid) == open.selected.as_ref())
        else {
            return;
        };
        let result = (|| -> anyhow::Result<bool> {
            let changed = item.title() != title || item.text() != body;
            if item.title() != title {
                item.set_title(&title)?;
            }
            if item.text() != body {
                item.set_text(&body)?;
            }
            Ok(changed)
        })();
        match result {
            Ok(true) => self.changed(tx),
            Ok(false) => {}
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn apply_tags(&mut self, tx: &mut Tx) {
        if !self.can_edit() {
            return;
        }
        let tags: Vec<String> = self
            .tags
            .text
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_owned)
            .collect();
        let Some(open) = self.open_mut() else {
            return;
        };
        let Some(id) = open.selected.clone() else {
            return;
        };
        if tag_names(&open.data, &id) == tags {
            return;
        }
        match set_note_tags(&mut open.data, &id, &tags) {
            Ok(()) => self.changed(tx),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn toggle_flag(&mut self, flag: &'static str, tx: &mut Tx) {
        if self.busy() {
            return;
        }
        let Some(open) = self.open_mut() else {
            return;
        };
        let Some(item) = open
            .data
            .items
            .iter_mut()
            .find(|item| Some(&item.uuid) == open.selected.as_ref())
        else {
            return;
        };
        if item.content_type != "Note" {
            return;
        }
        match item.set_flag(flag, !item.flag(flag)) {
            Ok(()) => self.changed(tx),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn changed(&mut self, tx: &mut Tx) {
        let demo = self.demo;
        if let Some(open) = self.open_mut() {
            open.revision = open.revision.saturating_add(1);
            if demo {
                open.save = SavePhase::Clean;
            } else if !matches!(open.save, SavePhase::Writing { .. }) {
                let due = tx.clock() + 0.65;
                open.save = SavePhase::Waiting { due };
                tx.wake_at(due);
            }
        }
        self.status = if demo {
            "Demonstração · somente em memória"
        } else {
            "Alterações aguardando salvamento…"
        }
        .into();
        self.refresh();
    }

    pub fn save_now(&mut self) {
        if self.demo {
            return;
        }
        let Some(open) = self.open_mut() else {
            return;
        };
        if matches!(open.save, SavePhase::Writing { .. }) {
            return;
        }
        let revision = open.revision;
        let data = open.data.clone();
        open.save = SavePhase::Writing { revision };
        self.status = "Salvando cofre…".into();
        if !self.send(Command::Save { revision, data })
            && let Some(open) = self.open_mut()
        {
            open.save = SavePhase::Failed;
        }
    }

    pub fn request_lock(&mut self, tx: &mut Tx) {
        tx.clear_secret_clipboard();
        self.apply_tags(tx);
        if self.io != IoPhase::Idle || self.busy() {
            return;
        }
        if self.demo {
            self.clear_locked();
            return;
        }
        let previous = std::mem::replace(&mut self.session, Session::Locked);
        match previous {
            Session::Open(mut open) => {
                if matches!(open.save, SavePhase::Writing { .. }) {
                    self.session = Session::Locking(open);
                } else if matches!(open.save, SavePhase::Clean) {
                    self.session = Session::Locking(open);
                    self.send(Command::Lock);
                } else {
                    let revision = open.revision;
                    let data = open.data.clone();
                    open.save = SavePhase::Writing { revision };
                    self.session = Session::Locking(open);
                    self.send(Command::Save { revision, data });
                }
                self.status = "Protegendo o cofre…".into();
            }
            other => self.session = other,
        }
    }

    pub fn clear_locked(&mut self) {
        self.account_ui.clear();
        self.backup_ui.clear();
        match &mut self.session {
            Session::Open(open) | Session::Locking(open) => open.data.clear_sensitive(),
            _ => {}
        }
        self.session = Session::Locked;
        self.create = false;
        self.password.clear();
        self.confirmation.clear();
        self.search.clear();
        self.title.clear();
        self.body.clear();
        self.tags.clear();
        self.confirm_export = false;
        self.status = "Cofre bloqueado".into();
    }

    pub fn toggle_theme(&mut self, tx: &mut Tx) {
        self.dark = !self.dark;
        let dark = self.dark;
        if let Some(open) = self.open_mut() {
            open.data.settings.insert("dark".into(), json!(dark));
            self.changed(tx);
        }
    }

    pub fn import_dialog(&mut self, tx: &mut Tx) {
        if self.demo || self.io != IoPhase::Idle || self.busy() {
            return;
        }
        let options = FileDialogOptions {
            title: "Importar backup ou texto".into(),
            ..Default::default()
        }
        .filter("Backup / texto / Markdown", &["sn", "json", "txt", "md"]);
        let generation = self.begin_dialog();
        let original_path = self.path.clone();
        tx.open_file(options, move |s: &mut AppState, result, _| {
            if !s.finish_dialog(generation) || s.path != original_path || s.open().is_none() {
                return;
            }
            match result {
                Ok(Some(path)) => {
                    if s.send(Command::Import { path }) {
                        s.io = IoPhase::Importing;
                        s.status = "Lendo importação…".into();
                    }
                }
                Ok(None) => {}
                Err(error) => s.error = Some(error.to_string()),
            }
        });
    }

    pub fn export_dialog(&mut self, tx: &mut Tx) {
        self.confirm_export = false;
        if self.demo || self.io != IoPhase::Idle || self.busy() {
            return;
        }
        self.apply_tags(tx);
        let options = FileDialogOptions {
            title: "Exportar backup sem criptografia".into(),
            ..Default::default()
        }
        .filter("Standard Notes JSON", &["json"]);
        let generation = self.begin_dialog();
        let original_path = self.path.clone();
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
                        if s.send(Command::Export { path, document }) {
                            s.io = IoPhase::Exporting;
                            s.status = "Exportando backup…".into();
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => s.error = Some(error.to_string()),
            }
        });
    }

    pub fn flush_for_exit(&mut self) -> Result<(), String> {
        if self.demo {
            return Ok(());
        }
        let data = self
            .open()
            .filter(|open| !matches!(open.save, SavePhase::Clean))
            .map(|open| open.data.clone());
        if let Some(data) = data {
            self.worker.flush(data)?;
            if let Some(open) = self.open_mut() {
                open.save = SavePhase::Clean;
            }
        }
        Ok(())
    }

    pub fn handle(&mut self, event: Event, tx: &mut Tx) {
        match event {
            Event::Opened(data) => self.open_data(data),
            Event::Locked => self.clear_locked(),
            Event::Saved(revision) => {
                if !self.open().is_some_and(|open| matches!(open.save, SavePhase::Writing { revision: expected } if expected == revision)) {
                    return;
                }
                let previous = std::mem::replace(&mut self.session, Session::Locked);
                match previous {
                    Session::Open(mut open) => {
                        if open.revision == revision {
                            open.save = SavePhase::Clean;
                            self.status = "Tudo salvo neste dispositivo".into();
                        } else {
                            open.save = SavePhase::Waiting { due: tx.clock() };
                            tx.wake_after(std::time::Duration::ZERO);
                        }
                        self.session = Session::Open(open);
                    }
                    Session::Locking(mut open) => {
                        if open.revision == revision {
                            self.session = Session::Locking(open);
                            self.send(Command::Lock);
                        } else {
                            let revision = open.revision;
                            let data = open.data.clone();
                            open.save = SavePhase::Writing { revision };
                            self.session = Session::Locking(open);
                            self.send(Command::Save { revision, data });
                        }
                    }
                    other => self.session = other,
                }
            }
            Event::Imported(document) => {
                self.io = IoPhase::Idle;
                self.backup_ui.clear();
                if let Some(open) = self.open_mut() {
                    match merge_document(&mut open.data, document) {
                        Ok(report) => {
                            self.changed(tx);
                            self.status = format!(
                                "Importadas: {} · já existentes: {} · conflitos preservados: {}",
                                report.added, report.skipped, report.conflicts
                            );
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            }
            Event::EncryptedImportNeeded(path) => {
                self.io = IoPhase::Idle;
                self.backup_ui.import(path);
                self.error = None;
                self.status = "Informe a senha para importar o backup".into();
            }
            Event::Exported { encrypted } => {
                self.io = IoPhase::Idle;
                self.backup_ui.clear();
                self.status = if encrypted {
                    "Backup criptografado exportado"
                } else {
                    "Backup exportado sem criptografia"
                }
                .into();
            }
            Event::Failed { operation, message } => {
                // Autosave runs independently of file dialogs and import/export.
                // Its failure must not release a reservation owned by another job.
                if matches!(
                    (operation, &self.io),
                    ("import", IoPhase::Importing) | ("export", IoPhase::Exporting)
                ) {
                    self.io = IoPhase::Idle;
                }
                self.error = Some(message);
                let previous = std::mem::replace(&mut self.session, Session::Locked);
                self.session = match previous {
                    Session::Opening => Session::Locked,
                    Session::Open(mut open) | Session::Locking(mut open) if operation == "save" => {
                        open.save = SavePhase::Failed;
                        self.status = "Não salvo · tente novamente".into();
                        Session::Open(open)
                    }
                    other => other,
                };
            }
        }
    }
}

/// Unknown editors stay opaque. In particular a JSON Super Editor document must never be
/// fed through Rune's smaller HTML model and silently rewritten.
pub fn editable(item: &Item) -> bool {
    item.is_editable_text_source()
        && item.content.is_object()
        && !item.flag("trashed")
        && item
            .content
            .get("text")
            .is_some_and(|value| value.is_string())
}

pub fn poll(state: &mut AppState, now: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    while let Some(event) = state.worker.try_recv() {
        state.handle(event, tx);
        changed = true;
    }
    changed |= crate::account::poll(state, tx);
    let due = state.open().and_then(|open| match open.save {
        SavePhase::Waiting { due } => Some(due),
        _ => None,
    });
    if let Some(due) = due {
        if now >= due {
            state.save_now();
            changed = true;
        } else {
            tx.wake_at(due);
        }
    }
    if state.close_requested && !state.busy() && state.io == IoPhase::Idle {
        crate::closing(state, tx);
        changed = true;
    }
    changed
}
