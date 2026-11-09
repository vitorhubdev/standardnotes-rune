//! Interoperability with Standard Notes protocol 004.
//!
//! The wire formats and constants follow the upstream `packages/encryption` and
//! `packages/sncrypto-web` code pinned by this fork. This is not a replacement
//! for the server or an implementation of older protocols. Network operations
//! are blocking and must run outside the GUI thread.

mod account;
mod backup;
mod crypto;
mod sync;
mod wire;

pub use account::{AccountSession, login, logout};
pub use backup::{
    export_encrypted_backup, export_encrypted_document, import_encrypted_backup,
    import_encrypted_document,
};
pub use crypto::KeyParams;
pub use sync::{SyncReport, sync};

#[cfg(test)]
mod tests;
