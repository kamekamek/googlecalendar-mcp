use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AppConfig;

const CODE_EXPIRATION_SECS: i64 = 300;

#[derive(Debug)]
pub struct ProxyState {
    pub enabled: bool,
    pub public_url: String,
    pub registration_endpoint: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub protected_resource_metadata: String,
    pub openid_configuration: String,
    pub redirect_uri: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_auth_url: String,
    pub google_token_url: String,
    http_client: Client,
    clients: RwLock<HashMap<String, RegisteredClient>>,
    auth_states: RwLock<HashMap<String, AuthorizationRequest>>, // state -> request
    codes: RwLock<HashMap<String, AuthorizationCodeGrant>>,     // code -> grant
}

impl ProxyState {
    pub fn new(config: &AppConfig) -> Result<Self> {
        if !config.proxy.enabled {
            return Ok(Self::disabled());
        }

        let public_url = config.server.public_url.trim_end_matches('/').to_string();
        let redirect_path = config
            .proxy
            .redirect_path
            .clone()
            .unwrap_or_else(|| "/proxy/oauth/callback".to_string());
        let redirect_uri = format!("{}{}", public_url, redirect_path);

        let registration_endpoint = format!("{}/proxy/oauth/register", public_url);
        let authorization_endpoint = format!("{}/proxy/oauth/authorize", public_url);
        let token_endpoint = format!("{}/proxy/oauth/token", public_url);
        let protected_resource_metadata =
            format!("{}/.well-known/oauth-protected-resource", public_url);
        let openid_configuration = format!("{}/.well-known/openid-configuration", public_url);

        let http_client = Client::builder()
            .user_agent("mcp-google-calendar-proxy/0.1.0")
            .build()?;

        Ok(Self {
            enabled: true,
            public_url,
            registration_endpoint,
            authorization_endpoint,
            token_endpoint,
            protected_resource_metadata,
            openid_configuration,
            redirect_uri,
            google_client_id: config.oauth.client_id.clone(),
            google_client_secret: config.oauth.client_secret.clone(),
            google_auth_url: config.oauth.auth_url.clone(),
            google_token_url: config.oauth.token_url.clone(),
            http_client,
            clients: RwLock::new(HashMap::new()),
            auth_states: RwLock::new(HashMap::new()),
            codes: RwLock::new(HashMap::new()),
        })
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            public_url: String::new(),
            registration_endpoint: String::new(),
            authorization_endpoint: String::new(),
            token_endpoint: String::new(),
            protected_resource_metadata: String::new(),
            openid_configuration: String::new(),
            redirect_uri: String::new(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
            google_auth_url: String::new(),
            google_token_url: String::new(),
            http_client: Client::new(),
            clients: RwLock::new(HashMap::new()),
            auth_states: RwLock::new(HashMap::new()),
            codes: RwLock::new(HashMap::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn metadata(&self) -> AuthorizationServerMetadata {
        AuthorizationServerMetadata {
            issuer: self.public_url.clone(),
            authorization_endpoint: self.authorization_endpoint.clone(),
            token_endpoint: self.token_endpoint.clone(),
            registration_endpoint: self.registration_endpoint.clone(),
            response_types_supported: vec!["code"],
            grant_types_supported: vec!["authorization_code"],
            code_challenge_methods_supported: vec!["S256"],
            scopes_supported: vec!["https://www.googleapis.com/auth/calendar".to_string()],
            token_endpoint_auth_methods_supported: vec!["client_secret_post"],
            subject_types_supported: vec!["public"],
            id_token_signing_alg_values_supported: vec!["RS256"],
        }
    }

    pub fn protected_resource_metadata(&self, resource: String) -> ProtectedResourceMetadata {
        ProtectedResourceMetadata {
            resource,
            authorization_servers: vec![self.public_url.clone()],
            scopes_supported: vec!["https://www.googleapis.com/auth/calendar".to_string()],
        }
    }

    pub fn openid_configuration(&self) -> OpenIdConfiguration {
        OpenIdConfiguration {
            issuer: self.public_url.clone(),
            authorization_endpoint: self.authorization_endpoint.clone(),
            token_endpoint: self.token_endpoint.clone(),
            jwks_uri: format!("{}/.well-known/jwks.json", self.public_url),
            response_types_supported: vec!["code"],
            grant_types_supported: vec!["authorization_code"],
            code_challenge_methods_supported: vec!["S256"],
            scopes_supported: vec!["https://www.googleapis.com/auth/calendar".to_string()],
            token_endpoint_auth_methods_supported: vec!["client_secret_post"],
            subject_types_supported: vec!["public"],
            id_token_signing_alg_values_supported: vec!["RS256"],
        }
    }

    pub fn register_client(
        &self,
        request: ClientRegistrationRequest,
    ) -> Result<ClientRegistrationResponse> {
        if request.redirect_uris.is_empty() {
            return Err(anyhow!("redirect_uris is required"));
        }

        let client_id = Uuid::new_v4().to_string();
        let client_secret: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let record = RegisteredClient {
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            redirect_uris: request.redirect_uris.clone(),
            scope: request
                .scope
                .unwrap_or_else(|| "https://www.googleapis.com/auth/calendar".to_string()),
        };

        self.clients.write().insert(client_id.clone(), record);

        Ok(ClientRegistrationResponse {
            client_id,
            client_secret,
            client_id_issued_at: Utc::now().timestamp() as u64,
            client_secret_expires_at: 0,
            redirect_uris: request.redirect_uris,
            token_endpoint_auth_method: request
                .token_endpoint_auth_method
                .unwrap_or_else(|| "client_secret_post".to_string()),
        })
    }

    pub fn start_authorization(&self, params: &AuthorizationParams) -> Result<String> {
        let client = self
            .clients
            .read()
            .get(&params.client_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown client_id"))?;

        if !client.redirect_uris.contains(&params.redirect_uri) {
            return Err(anyhow!("redirect_uri is not registered"));
        }

        if params.response_type != "code" {
            return Err(anyhow!("unsupported response_type"));
        }

        let proxy_state = Uuid::new_v4().to_string();
        let original_state = params.state.clone();

        let scope = params.scope.clone().unwrap_or_else(|| client.scope.clone());

        self.auth_states.write().insert(
            proxy_state.clone(),
            AuthorizationRequest {
                client_id: params.client_id.clone(),
                redirect_uri: params.redirect_uri.clone(),
                original_state,
                code_challenge: params.code_challenge.clone(),
                code_challenge_method: params.code_challenge_method.clone(),
                scope,
            },
        );

        let mut google_url = reqwest::Url::parse(&self.google_auth_url)?;
        google_url
            .query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.google_client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", &self.merge_scopes(params))
            .append_pair("state", &proxy_state);

        if let Some(challenge) = &params.code_challenge {
            google_url
                .query_pairs_mut()
                .append_pair("code_challenge", challenge);
        }
        if let Some(method) = &params.code_challenge_method {
            google_url
                .query_pairs_mut()
                .append_pair("code_challenge_method", method);
        }

        Ok(google_url.into())
    }

    fn merge_scopes(&self, params: &AuthorizationParams) -> String {
        if let Some(scope) = &params.scope {
            scope.clone()
        } else {
            "https://www.googleapis.com/auth/calendar".to_string()
        }
    }

    pub fn handle_callback(&self, state: &str, code: &str) -> Result<CallbackResult> {
        let request = self
            .auth_states
            .write()
            .remove(state)
            .ok_or_else(|| anyhow!("state not found"))?;

        let proxy_code = Uuid::new_v4().to_string();

        self.codes.write().insert(
            proxy_code.clone(),
            AuthorizationCodeGrant {
                client_id: request.client_id.clone(),
                google_code: code.to_string(),
                redirect_uri: request.redirect_uri.clone(),
                scope: request.scope.clone(),
                created_at: Utc::now(),
            },
        );

        Ok(CallbackResult {
            proxy_code,
            redirect_uri: request.redirect_uri,
            original_state: request.original_state,
        })
    }

    pub async fn exchange_code(&self, form: &TokenRequest) -> Result<TokenResponse> {
        let client = self
            .clients
            .read()
            .get(&form.client_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown client_id"))?;

        if client.client_secret != form.client_secret {
            return Err(anyhow!("invalid client_secret"));
        }

        let grant = self
            .codes
            .write()
            .remove(&form.code)
            .ok_or_else(|| anyhow!("invalid or expired code"))?;

        if grant.created_at + chrono::Duration::seconds(CODE_EXPIRATION_SECS) < Utc::now() {
            return Err(anyhow!("authorization code expired"));
        }

        if !client.redirect_uris.contains(&form.redirect_uri) {
            return Err(anyhow!("redirect_uri mismatch"));
        }

        let mut request = vec![
            ("grant_type", "authorization_code"),
            ("code", grant.google_code.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("client_id", self.google_client_id.as_str()),
            ("client_secret", self.google_client_secret.as_str()),
        ];

        if let Some(verifier) = &form.code_verifier {
            request.push(("code_verifier", verifier.as_str()));
        }

        let response = self
            .http_client
            .post(&self.google_token_url)
            .form(&request)
            .send()
            .await?
            .error_for_status()?;

        let body: serde_json::Value = response.json().await?;

        Ok(TokenResponse { raw: body })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct RegisteredClient {
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
    scope: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AuthorizationRequest {
    client_id: String,
    redirect_uri: String,
    original_state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    scope: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AuthorizationCodeGrant {
    client_id: String,
    google_code: String,
    redirect_uri: String,
    scope: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct CallbackResult {
    pub proxy_code: String,
    pub redirect_uri: String,
    pub original_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClientRegistrationRequest {
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClientRegistrationResponse {
    pub client_id: String,
    pub client_secret: String,
    pub client_id_issued_at: u64,
    pub client_secret_expires_at: u64,
    pub redirect_uris: Vec<String>,
    pub token_endpoint_auth_method: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizationParams {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub code_challenge: Option<String>,
    #[serde(default)]
    pub code_challenge_method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub code_verifier: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
    pub response_types_supported: Vec<&'static str>,
    pub grant_types_supported: Vec<&'static str>,
    pub code_challenge_methods_supported: Vec<&'static str>,
    pub scopes_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<&'static str>,
    pub subject_types_supported: Vec<&'static str>,
    pub id_token_signing_alg_values_supported: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct ProtectedResourceMetadata {
    pub resource: String,
    pub authorization_servers: Vec<String>,
    pub scopes_supported: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OpenIdConfiguration {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub response_types_supported: Vec<&'static str>,
    pub grant_types_supported: Vec<&'static str>,
    pub code_challenge_methods_supported: Vec<&'static str>,
    pub scopes_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<&'static str>,
    pub subject_types_supported: Vec<&'static str>,
    pub id_token_signing_alg_values_supported: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub raw: serde_json::Value,
}
