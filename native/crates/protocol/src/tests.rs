use super::*;
use crate::account::{AccountSession, Tokens};
use crate::crypto::{RootKey, derive_root};
use crate::wire::{KeyRing, WireItem, protocol_timestamp};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sn_rune_core::{Item, VaultData};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const GOLDEN: &str = include_str!("../tests/fixtures/libsodium-004.sn");
const EXPECTED: &str = include_str!("../tests/fixtures/libsodium-004-expected.json");
fn expected() -> Value {
    serde_json::from_str(EXPECTED).unwrap()
}
fn golden() -> Value {
    serde_json::from_str(GOLDEN).unwrap()
}
fn fixture_root() -> RootKey {
    RootKey {
        master_key: expected()["master_key"].as_str().unwrap().into(),
        server_password: expected()["server_password"].as_str().unwrap().into(),
        params: serde_json::from_value(golden()["keyParams"].clone()).unwrap(),
    }
}
fn fixture_session(server: String) -> AccountSession {
    AccountSession {
        server,
        email: "fixture@example.invalid".into(),
        user_uuid: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
        root: fixture_root(),
        tokens: Tokens {
            access_token: "synthetic-access".into(),
            refresh_token: "synthetic-refresh".into(),
            access_expiration: chrono::Utc::now().timestamp_millis() + 3_600_000,
            refresh_expiration: 0,
            readonly_access: false,
        },
        sync_token: None,
    }
}

#[test]
fn root_derivation_matches_original_libsodium_fixture() {
    let params = serde_json::from_value(golden()["keyParams"].clone()).unwrap();
    let result = derive_root(expected()["password"].as_str().unwrap(), &params).unwrap();
    assert_eq!(result.master_key, expected()["master_key"]);
    assert_eq!(result.server_password, expected()["server_password"]);
}

#[test]
fn argon_matches_upstream_unicode_vector() {
    use argon2::{Algorithm, Argon2, Params, Version};
    let params = Params::new(65_536, 2, 1, Some(16)).unwrap();
    let mut output = [0u8; 16];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(
            "correct horse battery staple ✅".as_bytes(),
            &hex::decode("808182838485868788898a8b8c8d8e8f").unwrap(),
            &mut output,
        )
        .unwrap();
    // packages/sncrypto-web/test/crypto.test.js (predefined salt test).
    assert_eq!(hex::encode(output), "18dfbc268f251701652c8e38b5273f73");
}

#[test]
fn imports_libsodium_backup_without_losing_unknown_content() {
    let items = import_encrypted_backup(GOLDEN, expected()["password"].as_str().unwrap()).unwrap();
    assert_eq!(items.len(), 4);
    for (actual, expected) in items.iter().zip(expected()["items"].as_array().unwrap()) {
        assert_eq!(actual.content, expected["content"]);
        assert_eq!(actual.uuid, expected["uuid"]);
        assert!(actual.dirty);
    }
    assert_eq!(
        items[1].extra["extension_metadata"],
        json!({"unfamiliar":true})
    );
    assert_eq!(
        items[1].extra["updated_at_timestamp"],
        json!(1794225600123456i64)
    );
}

#[test]
fn private_vault_key_hierarchy_matches_libsodium_and_survives_new_backup_password() {
    let backup = include_str!("../tests/fixtures/libsodium-004-private-vault.sn");
    let mut value: Value = serde_json::from_str(backup).unwrap();
    value["vendor_metadata"] = json!({"client":"fixture","unfamiliar":[1,true,"á"]});
    let document =
        import_encrypted_document(&value.to_string(), expected()["password"].as_str().unwrap())
            .unwrap();
    assert_eq!(document.items.len(), 7);
    let note = document
        .items
        .iter()
        .find(|i| i.uuid == "77777777-7777-4777-8777-777777777777")
        .unwrap();
    assert_eq!(note.text(), "Hierarquia de chaves preservada 🔐");
    assert_eq!(note.extra["key_system_identifier"], "private-vault-fixture");
    assert!(!document.metadata.contains_key("keyParams"));
    let exported = export_encrypted_document(&document, "another synthetic passphrase").unwrap();
    let roundtrip = import_encrypted_document(&exported, "another synthetic passphrase").unwrap();
    assert_eq!(roundtrip.items.len(), 7);
    assert_eq!(
        roundtrip.metadata["vendor_metadata"],
        value["vendor_metadata"]
    );
    for item in &document.items {
        let after = roundtrip
            .items
            .iter()
            .find(|i| i.uuid == item.uuid)
            .unwrap();
        assert_eq!(after.content, item.content);
    }
}

#[test]
fn incorrect_password_or_tampered_ciphertext_rejects_entire_backup() {
    assert!(import_encrypted_backup(GOLDEN, "wrong").is_err());
    let mut altered = golden();
    let cipher = altered["items"][1]["content"].as_str().unwrap().to_owned();
    let mut parts: Vec<_> = cipher.split(':').map(str::to_owned).collect();
    let mut bytes = STANDARD.decode(&parts[2]).unwrap();
    bytes[0] ^= 1;
    parts[2] = STANDARD.encode(bytes);
    altered["items"][1]["content"] = json!(parts.join(":"));
    assert!(
        import_encrypted_backup(
            &altered.to_string(),
            expected()["password"].as_str().unwrap()
        )
        .is_err()
    );
}

#[test]
fn authenticates_uuid_and_refuses_unverified_signatures() {
    let mut wire: Vec<WireItem> = serde_json::from_value(golden()["items"].clone()).unwrap();
    let mut ring = KeyRing::new(&fixture_root(), &[]).unwrap();
    ring.decrypt_all(&wire[..1], false).unwrap();
    wire[1].uuid = "99999999-9999-4999-8999-999999999999".into();
    assert!(ring.decrypt_all(&wire[1..2], false).is_err());
    let mut wire: Vec<WireItem> = serde_json::from_value(golden()["items"].clone()).unwrap();
    wire[1]
        .extra
        .insert("shared_vault_uuid".into(), json!("shared-vault"));
    assert!(ring.decrypt_all(&wire[1..2], false).is_err());
}

#[test]
fn encrypted_export_round_trip_and_plaintext_never_appears() {
    let mut note = Item::new_note("private title", "conteúdo sigiloso único");
    note.content["custom"] = json!({"nested":[1,2,{"other":"á"}]});
    let exported = export_encrypted_backup(&[note.clone()], "export test passphrase").unwrap();
    assert!(!exported.contains("private title"));
    assert!(!exported.contains("conteúdo sigiloso"));
    let imported = import_encrypted_backup(&exported, "export test passphrase").unwrap();
    assert_eq!(
        imported
            .iter()
            .find(|item| item.uuid == note.uuid)
            .unwrap()
            .content,
        note.content
    );
    assert!(
        imported
            .iter()
            .any(|item| item.content_type == "SN|ItemsKey")
    );
    assert!(export_encrypted_backup(&[note], "").is_err());
}

#[test]
fn duplicate_identifiers_reject_import_before_decryption() {
    let mut backup = golden();
    let duplicate = backup["items"][1].clone();
    backup["items"].as_array_mut().unwrap().push(duplicate);
    assert!(
        import_encrypted_backup(&backup.to_string(), "irrelevant")
            .unwrap_err()
            .to_string()
            .contains("duplicados")
    );
}

#[test]
fn preserves_microsecond_conflict_revision_and_normalizes_proven_milliseconds() {
    let date = "2026-11-09T12:00:00.123Z";
    assert_eq!(
        protocol_timestamp(date, Some(&json!(1794225600123456i64))).unwrap(),
        1794225600123456i64
    );
    assert_eq!(
        protocol_timestamp(date, Some(&json!(1794225600123i64))).unwrap(),
        1794225600123000i64
    );
    assert!(protocol_timestamp(date, Some(&json!(12345))).is_err());
}

#[test]
fn login_uses_exact_pkce_and_never_sends_the_account_password() {
    let challenge = Arc::new(Mutex::new(String::new()));
    let capture = challenge.clone();
    let server = MockServer::new(vec![
        Box::new(move |r| {
            assert_eq!(r.path, "/v2/login-params");
            assert_eq!(r.body["email"], "fixture@example.invalid");
            assert_eq!(r.body["mfa_code"], "123456");
            *capture.lock().unwrap() = r.body["code_challenge"].as_str().unwrap().into();
            (200, json!({"data":golden()["keyParams"]}))
        }),
        Box::new(move |r| {
            assert_eq!(r.path, "/v2/login");
            assert_ne!(r.body["password"], expected()["password"]);
            assert_eq!(r.body["password"], expected()["server_password"]);
            let verifier = r.body["code_verifier"].as_str().unwrap();
            let calculated = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(hex::encode(Sha256::digest(verifier.as_bytes())));
            assert_eq!(calculated, *challenge.lock().unwrap());
            (
                200,
                json!({"data":{"session":{"access_token":"test-a","refresh_token":"test-r","access_expiration":9999999999999i64,"refresh_expiration":9999999999999i64,"readonly_access":false},"user":{"uuid":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"}}}),
            )
        }),
    ]);
    let session = login(
        &server.url,
        "  FIXTURE@example.invalid  ",
        expected()["password"].as_str().unwrap(),
        Some("123456"),
    )
    .unwrap();
    assert_eq!(session.email, "fixture@example.invalid");
    assert!(!session.is_read_only());
    server.finish();
}

#[test]
fn paginated_download_decrypts_items_even_when_the_key_is_on_a_later_page() {
    let server = MockServer::new(vec![
        Box::new(|r| {
            assert_eq!(r.body["items"], json!([]));
            assert!(r.auth.ends_with("synthetic-access"));
            (
                200,
                json!({"data":{"retrieved_items":&golden()["items"].as_array().unwrap()[1..],"sync_token":"t1","cursor_token":"page2"}}),
            )
        }),
        Box::new(|r| {
            assert_eq!(r.body["cursor_token"], "page2");
            assert_eq!(r.body["sync_token"], "t1");
            (
                200,
                json!({"retrieved_items":[golden()["items"][0]],"sync_token":"t2","cursor_token":null}),
            )
        }),
    ]);
    let mut session = fixture_session(server.url.clone());
    let mut data = VaultData::default();
    let report = sync(&mut session, &mut data).unwrap();
    assert_eq!(data.items.len(), 4);
    assert_eq!(report.downloaded, 4);
    assert_eq!(report.uploaded, 0);
    assert!(data.items.iter().all(|i| !i.dirty));
    assert_eq!(session.sync_token.as_deref(), Some("t2"));
    server.finish();
}

#[test]
fn missing_ack_keeps_local_data_dirty_and_does_not_advance_cursor() {
    let server = MockServer::new(vec![
        Box::new(|_| {
            (
                200,
                json!({"retrieved_items":[golden()["items"][0]],"sync_token":"pull"}),
            )
        }),
        Box::new(|r| {
            assert_eq!(r.body["items"].as_array().unwrap().len(), 1);
            assert!(
                r.body["items"][0]["content"]
                    .as_str()
                    .unwrap()
                    .starts_with("004:")
            );
            (200, json!({"saved_items":[],"sync_token":"unconfirmed"}))
        }),
    ]);
    let mut session = fixture_session(server.url.clone());
    let mut data = VaultData::default();
    data.items.push(Item::new_note("Local", "preserve me"));
    let before = serde_json::to_value(&data).unwrap();
    assert!(sync(&mut session, &mut data).is_err());
    assert_eq!(serde_json::to_value(data).unwrap(), before);
    assert!(session.sync_token.is_none());
    server.finish();
}

#[test]
fn partial_remote_write_failure_leaves_all_local_items_retryable() {
    let server = MockServer::new(vec![
        Box::new(|_| {
            (
                200,
                json!({"retrieved_items":[golden()["items"][0]],"sync_token":"pull"}),
            )
        }),
        Box::new(|r| {
            assert_eq!(r.body["items"].as_array().unwrap().len(), 150);
            let saved:Vec<_>=r.body["items"].as_array().unwrap().iter().map(|i|json!({"uuid":i["uuid"],"content_type":i["content_type"],"created_at":i["created_at"],"updated_at":"2026-11-09T13:00:00.000Z","deleted":false})).collect();
            (
                200,
                json!({"saved_items":saved,"sync_token":"partially-saved"}),
            )
        }),
        Box::new(|r| {
            assert_eq!(r.body["items"].as_array().unwrap().len(), 1);
            (503, json!({"error":{"message":"temporary"}}))
        }),
    ]);
    let mut session = fixture_session(server.url.clone());
    let mut data = VaultData {
        items: (0..151)
            .map(|i| Item::new_note(&format!("Note {i}"), "preserve unsent state"))
            .collect(),
        ..VaultData::default()
    };
    let before = serde_json::to_value(&data).unwrap();
    assert!(sync(&mut session, &mut data).is_err());
    assert_eq!(serde_json::to_value(data).unwrap(), before);
    assert!(session.sync_token.is_none());
    server.finish();
}

#[test]
fn refresh_tokens_survive_a_later_sync_failure_without_advancing_data() {
    let server = MockServer::new(vec![
        Box::new(|_| (401, json!({"error":{"tag":"invalid-auth"}}))),
        Box::new(|r| {
            assert_eq!(r.path, "/v1/sessions/refresh");
            assert_eq!(r.body["refresh_token"], "synthetic-refresh");
            (
                200,
                json!({"session":{"access_token":"new-access","refresh_token":"new-refresh","access_expiration":9999999999999i64,"refresh_expiration":9999999999999i64,"readonly_access":false}}),
            )
        }),
        Box::new(|r| {
            assert!(r.auth.ends_with("new-access"));
            (503, json!({"error":{}}))
        }),
    ]);
    let mut session = fixture_session(server.url.clone());
    let mut data = VaultData::default();
    assert!(sync(&mut session, &mut data).is_err());
    assert!(data.items.is_empty());
    assert!(session.sync_token.is_none());
    assert_eq!(session.tokens.refresh_token, "new-refresh");
    server.finish();
}

#[test]
fn conflict_keeps_both_note_versions_and_tag_memberships() {
    let mut local = Item::new_note("Original title", "offline changes");
    local.updated_at = "2026-10-01T00:00:00Z".into();
    local.server_updated_at = Some(local.updated_at.clone());
    let mut tag = Item::new_tag("Work");
    tag.content["references"] = json!([{"uuid":local.uuid,"content_type":"Note"}]);
    let mut remote = local.clone();
    remote.content["text"] = json!("server changes");
    remote.updated_at = "2026-11-01T00:00:00Z".into();
    remote.server_updated_at = Some(remote.updated_at.clone());
    remote.dirty = false;
    let original = local.uuid.clone();
    let mut data = VaultData {
        items: vec![local, tag],
        ..VaultData::default()
    };
    let mut report = SyncReport::default();
    crate::sync::merge_remote(&mut data, vec![remote], &mut report).unwrap();
    assert_eq!(report.conflicts, 1);
    assert_eq!(
        data.items
            .iter()
            .find(|i| i.uuid == original)
            .unwrap()
            .text(),
        "server changes"
    );
    let duplicate = data
        .items
        .iter()
        .find(|i| i.content.get("conflict_of") == Some(&json!(original)))
        .unwrap();
    assert_eq!(duplicate.text(), "offline changes");
    assert!(duplicate.dirty);
    assert_eq!(
        data.items[1].content["references"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn rejects_plain_http_remote_servers_and_credentials_in_urls() {
    assert!(crate::account::validate_server("http://example.com").is_err());
    assert!(crate::account::validate_server("https://user:password@example.com").is_err());
    assert!(crate::account::validate_server("https://example.com/?token=secret").is_err());
    assert_eq!(
        crate::account::validate_server("https://api.standardnotes.com/").unwrap(),
        "https://api.standardnotes.com"
    );
}

#[test]
fn standardnotes_498_expiry_refreshes_then_retries_the_same_read_once() {
    let server = MockServer::new(vec![
        Box::new(|r| {
            assert_eq!(r.path, "/v1/items");
            (498, json!({"error":{"tag":"expired-access-token"}}))
        }),
        Box::new(|r| {
            assert_eq!(r.path, "/v1/sessions/refresh");
            (
                200,
                json!({"session":{"access_token":"renewed-a","refresh_token":"renewed-r","access_expiration":9999999999999i64,"readonly_access":false}}),
            )
        }),
        Box::new(|r| {
            assert_eq!(r.path, "/v1/items");
            assert!(r.auth.ends_with("renewed-a"));
            assert_eq!(r.body["items"], json!([]));
            (
                200,
                json!({"retrieved_items":[golden()["items"][0]],"sync_token":"after-refresh"}),
            )
        }),
    ]);
    let mut session = fixture_session(server.url.clone());
    let mut data = VaultData::default();
    let report = sync(&mut session, &mut data).unwrap();
    assert_eq!(report.downloaded, 1);
    assert_eq!(session.tokens.refresh_token, "renewed-r");
    assert_eq!(session.sync_token.as_deref(), Some("after-refresh"));
    server.finish();
}

#[test]
fn rejects_plaintext_remote_payload_before_any_upload_or_local_commit() {
    let note = Item::new_note("existing", "local authentic content");
    let id = note.uuid.clone();
    let server = MockServer::new(vec![Box::new(move |r| {
        assert_eq!(r.body["items"], json!([]));
        (
            200,
            json!({"retrieved_items":[golden()["items"][0],{"uuid":id,"content_type":"Note","content":{"title":"injected","text":"unauthenticated server data"},"created_at":"2026-11-09T12:00:00Z","updated_at":"2026-11-09T13:00:00Z","deleted":false}],"sync_token":"must-not-commit"}),
        )
    })]);
    let mut session = fixture_session(server.url.clone());
    let mut data = VaultData::default();
    data.items.push(note);
    let before = serde_json::to_value(&data).unwrap();
    let error = sync(&mut session, &mut data).unwrap_err();
    assert!(error.to_string().contains("não criptografado"));
    assert_eq!(serde_json::to_value(data).unwrap(), before);
    assert!(session.sync_token.is_none());
    server.finish();
}

struct Request {
    path: String,
    auth: String,
    body: Value,
}
type Handler = Box<dyn FnOnce(Request) -> (u16, Value) + Send>;
struct MockServer {
    url: String,
    thread: Option<thread::JoinHandle<()>>,
}
impl MockServer {
    fn new(handlers: Vec<Handler>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            for handler in handlers {
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "expected request did not arrive");
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => panic!("mock accept: {e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let request = read_request(&mut stream);
                let (status, body) = handler(request);
                let body = body.to_string();
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        Self {
            url,
            thread: Some(handle),
        }
    }
    fn finish(mut self) {
        self.thread.take().unwrap().join().unwrap();
    }
}
fn read_request(stream: &mut TcpStream) -> Request {
    let mut buffer = Vec::new();
    let mut byte = [0];
    while !buffer.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        buffer.push(byte[0]);
        assert!(buffer.len() < 65536);
    }
    let headers = String::from_utf8(buffer).unwrap();
    let path = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_string();
    let mut length = 0;
    let mut auth = String::new();
    for header in headers.lines().skip(1) {
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap();
            }
            if name.eq_ignore_ascii_case("authorization") {
                auth = value.trim().into();
            }
        }
    }
    assert!(length < 8 * 1024 * 1024);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    Request {
        path,
        auth,
        body: serde_json::from_slice(&body).unwrap(),
    }
}
