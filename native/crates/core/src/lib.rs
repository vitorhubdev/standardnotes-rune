//! Native data and storage layer. The local vault format is deliberately separate
//! from Standard Notes' network encryption protocols.
//!
//! The caller owns decrypted `VaultData` and must drop/clear it when locking the
//! UI. Dropping `Vault` releases its process lock and zeroizes the derived key.

mod backup;
mod model;
mod vault;

pub use backup::{
    BackupDocument, ImportReport, export_backup, export_document, import_backup, import_document,
    import_file, import_text, merge_document, merge_import,
};
pub use model::{APP_DOMAIN, Item, VaultData, set_note_tags, tag_names};
pub use vault::Vault;
