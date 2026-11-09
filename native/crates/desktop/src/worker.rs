//! The vault and its key never leave this serial worker. Disk and key derivation do not run
//! in a UI event handler. Revisions let the UI distinguish an old save from its latest edit.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use rune::Waker;
use sn_rune_core::{
    BackupDocument, Vault, VaultData, export_document, import_document, import_file,
};
use zeroize::Zeroizing;

pub enum Command {
    Open {
        path: PathBuf,
        password: Zeroizing<String>,
        create: bool,
    },
    Save {
        revision: u64,
        data: VaultData,
    },
    Import {
        path: PathBuf,
    },
    ImportEncrypted {
        path: PathBuf,
        password: Zeroizing<String>,
    },
    Export {
        path: PathBuf,
        document: BackupDocument,
    },
    ExportEncrypted {
        path: PathBuf,
        document: BackupDocument,
        password: Zeroizing<String>,
    },
    Lock,
    Flush {
        data: VaultData,
        answer: mpsc::SyncSender<Result<(), String>>,
    },
}

pub enum Event {
    Opened(VaultData),
    Saved(u64),
    Imported(BackupDocument),
    EncryptedImportNeeded(PathBuf),
    Exported {
        encrypted: bool,
    },
    Locked,
    Failed {
        operation: &'static str,
        message: String,
    },
}

pub struct Worker {
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    wake: Arc<Mutex<Option<Waker>>>,
}

impl Worker {
    pub fn new() -> Self {
        let (commands, requests) = mpsc::channel();
        let (answers, events) = mpsc::channel();
        let wake: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let thread_wake = wake.clone();
        std::thread::Builder::new()
            .name("notes-vault".into())
            .spawn(move || {
                let mut vault: Option<Vault> = None;
                while let Ok(command) = requests.recv() {
                    let event = process(command, &mut vault);
                    if let Some(event) = event {
                        if answers.send(event).is_err() {
                            break;
                        }
                        if let Ok(guard) = thread_wake.lock()
                            && let Some(wake) = guard.as_ref()
                        {
                            wake();
                        }
                    }
                }
            })
            .expect("could not start the vault worker");
        Self {
            commands,
            events,
            wake,
        }
    }

    pub fn set_waker(&self, wake: Waker) {
        if let Ok(mut current) = self.wake.lock() {
            *current = Some(wake);
        }
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "O serviço de armazenamento parou. Reinicie o aplicativo.".into())
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Rune's shutdown hook requires a synchronous flush before the process can end. The
    /// worker still owns all I/O; normal editing and the Lock button never block the UI.
    pub fn flush(&self, data: VaultData) -> Result<(), String> {
        let (answer, wait) = mpsc::sync_channel(1);
        self.send(Command::Flush { data, answer })?;
        wait.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "O salvamento final não foi confirmado em 15 segundos.".to_string())?
    }
}

fn process(command: Command, vault: &mut Option<Vault>) -> Option<Event> {
    let fail = |operation, error: anyhow::Error| Event::Failed {
        operation,
        message: error.to_string(),
    };
    Some(match command {
        Command::Open {
            path,
            password,
            create,
        } => {
            let opened = if create {
                Vault::create(&path, &password)
            } else {
                Vault::open(&path, &password)
            };
            match opened {
                Ok((opened, data)) => {
                    *vault = Some(opened);
                    Event::Opened(data)
                }
                Err(error) => fail("open", error),
            }
        }
        Command::Save { revision, data } => match vault.as_mut() {
            Some(vault) => match vault.save(&data) {
                Ok(()) => Event::Saved(revision),
                Err(error) => fail("save", error),
            },
            None => Event::Failed {
                operation: "save",
                message: "O cofre está bloqueado.".into(),
            },
        },
        Command::Import { path } => match read_import(&path) {
            Ok(Some(document)) => Event::Imported(document),
            Ok(None) => Event::EncryptedImportNeeded(path),
            Err(error) => fail("import", error),
        },
        Command::ImportEncrypted { path, password } => {
            match read_encrypted_import(&path, &password) {
                Ok(document) => Event::Imported(document),
                Err(error) => fail("import", error),
            }
        }
        Command::Export { path, document } => {
            let result = export_document(&document)
                .and_then(|json| write_new_backup(&path, Zeroizing::new(json)));
            match result {
                Ok(()) => Event::Exported { encrypted: false },
                Err(error) => fail("export", error),
            }
        }
        Command::ExportEncrypted {
            path,
            document,
            password,
        } => {
            let result = sn_rune_protocol::export_encrypted_document(&document, &password)
                .and_then(|json| write_new_backup(&path, Zeroizing::new(json)));
            match result {
                Ok(()) => Event::Exported { encrypted: true },
                Err(error) => fail("export", error),
            }
        }
        Command::Lock => {
            *vault = None;
            Event::Locked
        }
        Command::Flush { data, answer } => {
            let result = vault
                .as_mut()
                .ok_or_else(|| "O cofre está bloqueado.".to_string())
                .and_then(|vault| vault.save(&data).map_err(|error| error.to_string()));
            let _ = answer.send(result);
            return None;
        }
    })
}

fn read_import(path: &std::path::Path) -> anyhow::Result<Option<BackupDocument>> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "json" || extension == "sn" {
        let content = read_backup_text(path)?;
        let value: serde_json::Value =
            serde_json::from_str(content.trim_start_matches('\u{feff}'))?;
        let encrypted = value.get("keyParams").is_some_and(|v| !v.is_null())
            || value.get("auth_params").is_some_and(|v| !v.is_null())
            || value
                .get("items")
                .and_then(|v| v.as_array())
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.get("content").is_some_and(|c| c.is_string()))
                });
        if encrypted {
            return Ok(None);
        }
        import_document(&content).map(Some)
    } else {
        Ok(Some(BackupDocument {
            items: import_file(path)?,
            ..Default::default()
        }))
    }
}

fn read_backup_text(path: &std::path::Path) -> anyhow::Result<Zeroizing<String>> {
    const LIMIT: u64 = 128 * 1024 * 1024;
    let file = std::fs::File::open(path)?;
    let mut content = Zeroizing::new(String::new());
    file.take(LIMIT + 1).read_to_string(&mut content)?;
    anyhow::ensure!(content.len() as u64 <= LIMIT, "O backup excede 128 MiB.");
    Ok(content)
}

fn read_encrypted_import(path: &std::path::Path, password: &str) -> anyhow::Result<BackupDocument> {
    let text = read_backup_text(path)?;
    sn_rune_protocol::import_encrypted_document(&text, password)
}

fn write_new_backup(path: &std::path::Path, json: Zeroizing<String>) -> anyhow::Result<()> {
    // An export can never truncate the active vault or an existing backup.
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        anyhow::anyhow!(
            "Não foi possível criar o backup: {error}. Escolha um nome de arquivo novo."
        )
    })?;
    let written = file
        .write_all(json.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);
    if written.is_err() {
        let _ = std::fs::remove_file(path);
    }
    Ok(written?)
}
