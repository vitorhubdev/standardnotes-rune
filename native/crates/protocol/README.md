# Standard Notes protocol 004 in Rust

This crate implements the account/backup wire format used by the original
Standard Notes client in this fork. It does not use a JavaScript runtime,
Electron, or a browser. Rune owns presentation; this crate owns interoperability.

## Implemented

- Protocol 004: Argon2id13 (64 MiB, five iterations, one lane, 64-byte output),
  SHA-256 salt derivation, and XChaCha20Poly1305 IETF authenticated encryption.
- Standard `.sn` encrypted backups, including backups from offline sessions with
  plaintext items keys. Import authenticates every encrypted item or fails the
  entire operation. Unknown content and envelope metadata can be preserved with
  `import_encrypted_document`/`export_encrypted_document`.
- The private-vault key hierarchy when the needed vault root keys are present.
- Login v2, the upstream PKCE encoding, optional TOTP, token refresh on HTTP 498
  (and compatible servers returning 401), and logout.
- Paginated private-item synchronization in batches of 150, explicit save
  acknowledgements, preservation of full microsecond server revisions, and
  conflict copies for concurrently edited notes.
- HTTPS via rustls; 15-second connect timeout and 60-second total request timeout.
  HTTP is permitted only on loopback for local servers/tests. Redirects do not
  forward credentials. Mutating requests are not blindly retried after a
  timeout; the next download reconciles an uncertain write by decrypted content.

## Caller contract

All network and key-derivation functions block. Run them in the application's
worker, never in a Rune view or input callback.

`AccountSession` contains secret keys and tokens and intentionally has no Debug
implementation. Store `session.to_vault_value()` only in the encrypted
`VaultData.account` field. Local preferences and log files are inappropriate.

`sync(&mut session, &mut data)` stages downloaded items, acknowledgements, conflict
copies and cursor changes. An error leaves `data` unchanged, even if an earlier
remote batch was saved. **A refresh token can rotate before a later request
fails**: persist the updated `session` in the encrypted vault on both success and
failure. On success, persist the returned data and session together. The next
attempt detects already-applied remote changes without discarding local edits.

Only explicitly acknowledged items become clean. A server-provided plaintext
item cannot bypass decryption: plaintext is allowed in local backup import, not
remote synchronization. Unsupported items or missing keys stop the operation
before the initial upload. Non-note conflicts require explicit resolution and
return an error rather than choosing an arbitrary winner.

## Scope still requiring the original client

Protocols 001–003, signed/shared-vault payloads, WebAuthn and human-verification
challenges, account registration/recovery, changing account credentials, and
file-content upload/download are not implemented here. File/editor payload
metadata can be preserved; that is not equivalent to implementing the file or
editor service. A private vault whose root key is external/unavailable cannot be
decrypted. A connected account with shared-vault events is rejected rather than
partially synchronized.

## Evidence

`cargo test -p sn-rune-protocol` uses public synthetic fixtures generated with
the exact `libsodium-wrappers@0.7.10` pinned in the original client's cache.
Fixtures cover both the account items-key chain and a private-vault chain.
Tests also exercise a local HTTP mock for PKCE, MFA parameters, pagination,
expiry/refresh, missing acknowledgements, partial remote write failure, and
unauthenticated server content. A Rust-exported synthetic backup was additionally
decrypted using the original libsodium library and its authenticated-data
reconstruction. No real account credentials were used in these tests. Successful
fixtures and mock requests do not establish that every production account or
server configuration works.

See `tests/fixtures/README.md` for the generator and upstream source paths.
