use crate::crypto::{KeyParams, RootKey, derive_root, random_hex};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{StatusCode, Url, blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::Read, time::Duration};
use zeroize::Zeroize;

pub(crate) const API_VERSION: &str = "20240226";
const RESPONSE_LIMIT: u64 = 32 * 1024 * 1024;

/// Contains secrets. Serialize only inside the encrypted vault. No Debug impl.
#[derive(Clone, Serialize, Deserialize)]
pub struct AccountSession {
    pub server: String,
    pub email: String,
    pub user_uuid: String,
    pub(crate) root: RootKey,
    pub(crate) tokens: Tokens,
    #[serde(default)]
    pub(crate) sync_token: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub access_expiration: i64,
    #[serde(default)]
    pub refresh_expiration: i64,
    #[serde(default)]
    pub readonly_access: bool,
}

impl Drop for Tokens {
    fn drop(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}

impl AccountSession {
    pub fn is_read_only(&self) -> bool {
        self.tokens.readonly_access
    }
    pub fn has_synced(&self) -> bool {
        self.sync_token.is_some()
    }
    pub fn to_vault_value(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }
    pub fn from_vault_value(value: &Value) -> Result<Self> {
        let session: Self = serde_json::from_value(value.clone())
            .context("Sessão armazenada inválida; entre novamente.")?;
        validate_server(&session.server)?;
        ensure!(
            session.root.params.version == "004",
            "Sessão legada não suportada."
        );
        ensure!(
            !session.tokens.access_token.is_empty() && !session.tokens.refresh_token.is_empty(),
            "Sessão armazenada incompleta."
        );
        Ok(session)
    }
}

/// Authenticate using the v2 PKCE flow used by upstream, including optional TOTP.
/// The password stays local; only its derived server half is sent over TLS.
pub fn login(
    server: &str,
    email: &str,
    password: &str,
    mfa: Option<&str>,
) -> Result<AccountSession> {
    let server = validate_server(server)?;
    let email = email.trim().to_lowercase();
    ensure!(
        !email.is_empty() && !password.is_empty(),
        "Informe e-mail e senha da conta."
    );
    let client = Api::new(&server)?;
    let verifier = random_hex(32);
    // Upstream base64-encodes the UTF-8 hex digest, rather than raw digest bytes.
    let challenge =
        URL_SAFE_NO_PAD.encode(hex::encode(Sha256::digest(verifier.as_bytes())).as_bytes());
    let mut params = json!({"api":API_VERSION,"email":email,"code_challenge":challenge});
    if let Some(code) = mfa.filter(|s| !s.is_empty()) {
        params["mfa_code"] = json!(code.trim());
    }
    let mut key_response = client.post("/v2/login-params", &params, None)?;
    // Deliberately use the locally entered identifier, as the upstream client does.
    key_response["identifier"] = json!(email);
    let key_params: KeyParams = serde_json::from_value(key_response)
        .context("Servidor retornou parâmetros incompatíveis.")?;
    let mut root = derive_root(password, &key_params)?;
    let auth = json!({"api":API_VERSION,"email":email,"password":root.server_password,"ephemeral":false,"code_verifier":verifier});
    let response = client.post("/v2/login", &auth, None)?;
    let tokens: Tokens = serde_json::from_value(
        response
            .get("session")
            .cloned()
            .context("Servidor não retornou uma sessão compatível.")?,
    )
    .context("Sessão inválida.")?;
    ensure!(
        !tokens.access_token.is_empty() && !tokens.refresh_token.is_empty(),
        "Servidor retornou sessão incompleta."
    );
    let user_uuid = response
        .pointer("/user/uuid")
        .and_then(Value::as_str)
        .context("Servidor não identificou a conta.")?
        .to_owned();
    if let Some(updated) = response.get("key_params").filter(|v| v.is_object()) {
        let params: KeyParams = serde_json::from_value(updated.clone())
            .context("Servidor alterou os parâmetros de forma incompatível.")?;
        ensure!(
            params.version == root.params.version
                && params.pw_nonce == root.params.pw_nonce
                && params.identifier == root.params.identifier,
            "Parâmetros da conta mudaram durante o login; tente novamente."
        );
        root.params = params;
    }
    Ok(AccountSession {
        server,
        email,
        user_uuid,
        root,
        tokens,
        sync_token: None,
    })
}

pub fn logout(session: &mut AccountSession) -> Result<()> {
    let api = Api::new(&session.server)?;
    api.post(
        "/v1/logout",
        &json!({"api":API_VERSION}),
        Some(&session.tokens.access_token),
    )?;
    session.tokens.access_token.zeroize();
    session.tokens.refresh_token.zeroize();
    Ok(())
}

pub(crate) fn validate_server(server: &str) -> Result<String> {
    let url = Url::parse(server.trim()).context("URL de servidor inválida.")?;
    let localhost = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && localhost),
        "Use HTTPS para proteger a conta (HTTP somente em loopback local)."
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "URL não deve conter credenciais, consulta ou fragmento."
    );
    ensure!(url.host_str().is_some(), "Servidor sem endereço.");
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub(crate) struct Api {
    client: Client,
    server: String,
}

impl Api {
    pub fn new(server: &str) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(60))
                .redirect(Policy::none())
                .user_agent(concat!("StandardNotesRune/", env!("CARGO_PKG_VERSION")))
                .build()
                .context("Não foi possível iniciar o cliente HTTPS.")?,
            server: validate_server(server)?,
        })
    }

    pub fn post(&self, path: &str, payload: &Value, access_token: Option<&str>) -> Result<Value> {
        self.post_result(path, payload, access_token)
            .map_err(|error| error.into_anyhow())
    }

    pub fn authenticated_post(
        &self,
        path: &str,
        payload: &Value,
        session: &mut AccountSession,
    ) -> Result<Value> {
        // Refresh before expiry where possible. Standard Notes uses 498 for an
        // expired access token; 401 covers compatible/self-hosted servers. Either
        // unauthorized response is retried exactly once after a token refresh.
        if session.tokens.access_expiration > 0
            && session.tokens.access_expiration <= chrono::Utc::now().timestamp_millis() + 30_000
        {
            self.refresh(session)?;
        }
        match self.post_result(path, payload, Some(&session.tokens.access_token)) {
            Ok(response) => Ok(response),
            Err(ApiError {
                status: Some(status),
                ..
            }) if matches!(status.as_u16(), 401 | 498) => {
                self.refresh(session)?;
                self.post(path, payload, Some(&session.tokens.access_token))
            }
            Err(error) => Err(error.into_anyhow()),
        }
    }

    fn refresh(&self, session: &mut AccountSession) -> Result<()> {
        let response = self.post("/v1/sessions/refresh", &json!({"api":API_VERSION,"access_token":session.tokens.access_token,"refresh_token":session.tokens.refresh_token}), None)?;
        let tokens: Tokens = serde_json::from_value(
            response
                .get("session")
                .cloned()
                .context("Atualização de sessão inválida.")?,
        )?;
        ensure!(
            !tokens.access_token.is_empty() && !tokens.refresh_token.is_empty(),
            "Atualização de sessão incompleta."
        );
        session.tokens = tokens;
        Ok(())
    }

    fn post_result(
        &self,
        path: &str,
        payload: &Value,
        access_token: Option<&str>,
    ) -> std::result::Result<Value, ApiError> {
        let request = self
            .client
            .post(format!("{}{}", self.server, path))
            .json(payload)
            .header(
                "X-Application-Version",
                concat!("Rune-", env!("CARGO_PKG_VERSION")),
            );
        let request = if let Some(token) = access_token {
            request.bearer_auth(token)
        } else {
            request
        };
        let response = request.send().map_err(|_| ApiError::transport())?;
        let status = response.status();
        let mut bytes = Vec::new();
        response
            .take(RESPONSE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ApiError::transport())?;
        if bytes.len() as u64 > RESPONSE_LIMIT {
            return Err(ApiError {
                status: Some(status),
                message: "Resposta excede 32 MiB; nenhum dado local foi alterado.".into(),
            });
        }
        if status == StatusCode::NO_CONTENT {
            return Ok(json!({}));
        }
        let body: Value = serde_json::from_slice(&bytes).map_err(|_| ApiError {
            status: Some(status),
            message: "Servidor retornou uma resposta JSON inválida.".into(),
        })?;
        let data = body.get("data").cloned().unwrap_or(body);
        if !status.is_success() || data.get("error").is_some() {
            let tag = data
                .pointer("/error/tag")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let message = match tag {
                "mfa-required" | "mfa_required" => {
                    "A conta exige o código de autenticação em duas etapas."
                }
                "u2f-required" | "u2f_required" => {
                    "Esta conta exige uma chave de segurança/WebAuthn; use o aplicativo original."
                }
                "invalid-auth" | "invalid-credentials" => {
                    "Credenciais inválidas. Confira e-mail, senha e código de autenticação."
                }
                _ if status == StatusCode::TOO_MANY_REQUESTS => {
                    "Servidor limitou as tentativas. Aguarde antes de tentar novamente."
                }
                _ if matches!(status.as_u16(), 401 | 498) => {
                    "Sessão inválida ou expirada. Entre novamente."
                }
                _ => "O servidor recusou a operação. Confira os dados e tente novamente.",
            };
            return Err(ApiError {
                status: Some(status),
                message: message.into(),
            });
        }
        if !data.is_object() {
            return Err(ApiError {
                status: Some(status),
                message: "Resposta inesperada do servidor.".into(),
            });
        }
        Ok(data)
    }
}

struct ApiError {
    status: Option<StatusCode>,
    message: String,
}
impl ApiError {
    fn transport() -> Self {
        Self {status:None,message:"Falha de conexão ou tempo de espera esgotado. O resultado remoto pode ser incerto; sincronize novamente para conferir.".into()}
    }
    fn into_anyhow(self) -> anyhow::Error {
        if let Some(status) = self.status {
            anyhow::anyhow!("{} (HTTP {})", self.message, status.as_u16())
        } else {
            anyhow::anyhow!(self.message)
        }
    }
}
