use crate::mcp::{HttpMcpServer, ToolRequest, ToolResponse};
use crate::proxy::{
    AuthorizationParams, AuthorizationServerMetadata as ProxyMetadata, ClientRegistrationRequest,
    ClientRegistrationResponse, OpenIdConfiguration as ProxyOpenIdConfiguration,
    ProtectedResourceMetadata as ProxyResourceMetadata, TokenRequest,
};
use crate::{oauth::TokenInfo, AppState, AuthorizationSession};
use anyhow::Context;
use axum::extract::{Form, Path, Query};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Redirect};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration as StdDuration;

pub fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/callback", get(callback))
        .route("/mcp/tool", post(handle_tool));

    if state
        .proxy_state
        .as_ref()
        .map(|p| p.is_enabled())
        .unwrap_or(false)
    {
        router = router
            .route("/proxy/oauth/register", post(proxy_register_client))
            .route("/proxy/oauth/authorize", get(proxy_authorize))
            .route("/proxy/oauth/callback", get(proxy_callback))
            .route("/proxy/oauth/token", post(proxy_token))
            .route(
                "/.well-known/oauth-authorization-server",
                get(proxy_authorization_server_metadata),
            )
            .route(
                "/.well-known/oauth-protected-resource",
                get(proxy_protected_resource_metadata_root),
            )
            .route(
                "/.well-known/oauth-protected-resource/{*rest}",
                get(proxy_protected_resource_metadata_with_path),
            )
            .route(
                "/.well-known/openid-configuration",
                get(proxy_openid_configuration),
            );
    } else {
        router = router
            .route(
                "/.well-known/oauth-authorization-server",
                get(authorization_server_metadata),
            )
            .route(
                "/.well-known/oauth-protected-resource",
                get(protected_resource_metadata_root),
            )
            .route(
                "/.well-known/oauth-protected-resource/{*rest}",
                get(protected_resource_metadata_with_path),
            )
            .route(
                "/.well-known/openid-configuration",
                get(openid_configuration),
            );
    }

    router.layer(Extension(state))
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

#[derive(Debug, Deserialize)]
struct AuthorizeQuery {
    user_id: String,
    #[serde(default)]
    redirect_uri: Option<String>,
}

async fn authorize(
    Extension(state): Extension<Arc<AppState>>,
    Query(query): Query<AuthorizeQuery>,
) -> Result<impl IntoResponse, HandlerError> {
    cleanup_sessions(&state);

    let redirect_uri = query
        .redirect_uri
        .unwrap_or_else(|| state.config.oauth.redirect_uri.clone());
    let context = state
        .oauth_client
        .authorize_url(&redirect_uri)
        .context("failed to build authorization url")?;

    let session = AuthorizationSession {
        user_id: query.user_id.clone(),
        state: context.clone(),
        created_at: Utc::now(),
    };
    state
        .auth_sessions
        .write()
        .insert(session.state.csrf_state.clone(), session);

    Ok((StatusCode::OK, Json(context)))
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    state: String,
    code: String,
}

async fn callback(
    Extension(state): Extension<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Result<impl IntoResponse, HandlerError> {
    let session = {
        let mut sessions = state.auth_sessions.write();
        sessions.remove(&query.state)
    }
    .ok_or_else(|| HandlerError::unauthorized("invalid or expired state"))?;

    let token = state
        .oauth_client
        .exchange_code(
            &state.config.oauth.redirect_uri,
            &query.code,
            &session.state.pkce_verifier,
        )
        .await
        .context("failed to exchange authorization code")?;

    state
        .token_storage
        .persist(&session.user_id, &token)
        .await
        .context("failed to persist token")?;

    Ok((StatusCode::OK, Json(json!({ "status": "authorized" }))))
}

async fn handle_tool(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ToolRequest>,
) -> Result<impl IntoResponse, HandlerError> {
    if let Some(user_id) = request.user_id() {
        maybe_store_bearer_token(&state, &headers, user_id).await?;
    }

    if let Some(user_id) = request.user_id() {
        if state.token_storage.fetch(user_id).await?.is_none() {
            let context = state
                .oauth_client
                .authorize_url(&state.config.oauth.redirect_uri)?;
            let session = AuthorizationSession {
                user_id: user_id.to_string(),
                state: context.clone(),
                created_at: Utc::now(),
            };
            state
                .auth_sessions
                .write()
                .insert(session.state.csrf_state.clone(), session);
            let payload = json!({
                "authorize_url": context.authorize_url,
                "state": context.csrf_state,
                "pkce_verifier": context.pkce_verifier,
            });
            let response = ToolResponse {
                status: crate::mcp::ResponseStatus::Error,
                data: Some(payload),
                error: Some("authorization required".into()),
            };

            let metadata_url = if let Some(proxy) = state.proxy_state.as_ref() {
                proxy.protected_resource_metadata.clone()
            } else {
                protected_resource_metadata_url(&state.config.server.public_url)
            };
            let resource_id = state.config.server.public_url.trim_end_matches('/');
            let header_value = format!(
                "Bearer resource=\"{}\", resource_metadata=\"{}\"",
                resource_id, metadata_url
            );

            let mut response = (StatusCode::UNAUTHORIZED, Json(response)).into_response();
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_str(&header_value).map_err(|err| {
                    HandlerError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to construct WWW-Authenticate header",
                        Some(err.into()),
                    )
                })?,
            );

            return Ok(response);
        }
    }

    let server = HttpMcpServer::new(state);
    let response = server.handle_request(request).await;
    let status = match response.status {
        crate::mcp::ResponseStatus::Success => StatusCode::OK,
        crate::mcp::ResponseStatus::Error => StatusCode::BAD_REQUEST,
    };
    Ok((status, Json(response)).into_response())
}

fn cleanup_sessions(state: &Arc<AppState>) {
    let cutoff = Utc::now() - Duration::minutes(10);
    let mut sessions = state.auth_sessions.write();
    sessions.retain(|_, session| session.created_at > cutoff);
}

async fn maybe_store_bearer_token(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    user_id: &str,
) -> Result<(), HandlerError> {
    let Some(raw_header) = headers.get(AUTHORIZATION) else {
        return Ok(());
    };

    let header_value = raw_header.to_str().map_err(|err| {
        HandlerError::new(
            StatusCode::BAD_REQUEST,
            "authorization header must be valid UTF-8",
            Some(err.into()),
        )
    })?;

    let mut parts = header_value.splitn(2, ' ');
    let scheme = parts.next().unwrap_or_default();
    let token_part = parts.next().unwrap_or_default().trim();

    if !scheme.eq_ignore_ascii_case("Bearer") || token_part.is_empty() {
        return Ok(());
    }

    let token_value = token_part.to_owned();
    let (refresh_token, refresh_provided) = header_with_presence(
        headers,
        &["x-mcp-oauth-refresh-token", "x-oauth-refresh-token"],
    );
    let (scope, scope_provided) =
        header_with_presence(headers, &["x-mcp-oauth-scope", "x-oauth-scope"]);
    let (expires_at, expires_provided) = parse_expires_metadata(headers);
    let (token_type_header, _) =
        header_with_presence(headers, &["x-mcp-oauth-token-type", "x-oauth-token-type"]);
    let default_token_type = if scheme.eq_ignore_ascii_case("bearer") {
        "Bearer".to_owned()
    } else {
        scheme.to_owned()
    };
    let token_type = token_type_header.unwrap_or(default_token_type);

    let existing_token = state.token_storage.fetch(user_id).await?;
    let had_existing = existing_token.is_some();
    let mut token_info = existing_token.unwrap_or(TokenInfo {
        access_token: token_value.clone(),
        refresh_token: refresh_token.clone(),
        expires_at: expires_at.clone(),
        scope: scope.clone(),
        token_type: token_type.clone(),
    });

    let mut needs_persist = !had_existing;

    if token_info.access_token != token_value {
        token_info.access_token = token_value.clone();
        needs_persist = true;
    }

    if refresh_provided && token_info.refresh_token != refresh_token {
        token_info.refresh_token = refresh_token.clone();
        needs_persist = true;
    }

    if scope_provided && token_info.scope != scope {
        token_info.scope = scope.clone();
        needs_persist = true;
    }

    if expires_provided && token_info.expires_at != expires_at {
        token_info.expires_at = expires_at.clone();
        needs_persist = true;
    }

    if token_info.token_type != token_type {
        token_info.token_type = token_type.clone();
        needs_persist = true;
    }

    if needs_persist {
        state.token_storage.persist(user_id, &token_info).await?;
        if had_existing {
            tracing::info!(user_id = %user_id, "updated bearer token from Authorization header");
        } else {
            tracing::info!(user_id = %user_id, "stored bearer token from Authorization header");
        }
    }

    Ok(())
}

fn header_with_presence(headers: &HeaderMap, names: &[&'static str]) -> (Option<String>, bool) {
    for name in names {
        if let Some(value) = headers.get(*name) {
            match value.to_str() {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        return (None, true);
                    } else {
                        return (Some(trimmed.to_owned()), true);
                    }
                }
                Err(err) => {
                    tracing::warn!(header = *name, error = %err, "invalid header value");
                    return (None, true);
                }
            }
        }
    }

    (None, false)
}

fn parse_expires_metadata(headers: &HeaderMap) -> (Option<DateTime<Utc>>, bool) {
    const EXPIRES_AT_HEADERS: &[&str] = &["x-mcp-oauth-expires-at", "x-oauth-expires-at"];
    let (expires_at_raw, expires_at_present) = header_with_presence(headers, EXPIRES_AT_HEADERS);
    if expires_at_present {
        if let Some(raw) = expires_at_raw {
            if let Ok(parsed) = DateTime::parse_from_rfc3339(&raw) {
                return (Some(parsed.with_timezone(&Utc)), true);
            }

            if let Ok(epoch) = raw.parse::<i64>() {
                if let Some(datetime) = DateTime::<Utc>::from_timestamp(epoch, 0) {
                    return (Some(datetime), true);
                }
            }

            tracing::warn!(raw_expires_at = %raw, "failed to parse expires-at header");
        }

        return (None, false);
    }

    const EXPIRES_IN_HEADERS: &[&str] = &["x-mcp-oauth-expires-in", "x-oauth-expires-in"];
    let (expires_in_raw, expires_in_present) = header_with_presence(headers, EXPIRES_IN_HEADERS);
    if expires_in_present {
        if let Some(raw) = expires_in_raw {
            match raw.parse::<f64>() {
                Ok(seconds) if seconds.is_finite() && seconds >= 0.0 => {
                    let std_duration = StdDuration::from_secs_f64(seconds);
                    if let Ok(duration) = Duration::from_std(std_duration) {
                        return (Some(Utc::now() + duration), true);
                    }
                }
                Ok(_) => {
                    tracing::warn!(raw_expires_in = %raw, "expires-in header must be non-negative");
                }
                Err(err) => {
                    tracing::warn!(raw_expires_in = %raw, error = %err, "failed to parse expires-in header");
                }
            }
        }

        return (None, false);
    }

    (None, false)
}

fn issuer_from_auth_url(url: &str) -> Result<String, HandlerError> {
    let parsed = url::Url::parse(url).map_err(|err| {
        HandlerError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid authorization endpoint URL",
            Some(err.into()),
        )
    })?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().ok_or_else(|| {
        HandlerError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authorization endpoint missing host",
            None,
        )
    })?;
    let mut origin = format!("{}://{}", scheme, host);
    if let Some(port) = parsed.port() {
        origin.push_str(&format!(":{}", port));
    }
    Ok(origin)
}

fn protected_resource_metadata_url(public_url: &str) -> String {
    format!(
        "{}/.well-known/oauth-protected-resource",
        public_url.trim_end_matches('/')
    )
}

fn require_proxy_state(
    state: &Arc<AppState>,
) -> Result<Arc<crate::proxy::ProxyState>, HandlerError> {
    state
        .proxy_state
        .as_ref()
        .cloned()
        .ok_or_else(|| HandlerError::new(StatusCode::NOT_FOUND, "proxy not enabled", None))
}

async fn authorization_server_metadata(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<ProxyMetadata>, HandlerError> {
    let issuer = issuer_from_auth_url(&state.config.oauth.auth_url)?;
    Ok(Json(ProxyMetadata {
        issuer,
        authorization_endpoint: state.config.oauth.auth_url.clone(),
        token_endpoint: state.config.oauth.token_url.clone(),
        registration_endpoint: format!(
            "{}/proxy/oauth/register",
            state.config.server.public_url.trim_end_matches('/')
        ),
        response_types_supported: vec!["code"],
        grant_types_supported: vec!["authorization_code"],
        code_challenge_methods_supported: vec!["S256"],
        scopes_supported: state.config.oauth.scopes.clone(),
        token_endpoint_auth_methods_supported: vec!["client_secret_post"],
        subject_types_supported: vec!["public"],
        id_token_signing_alg_values_supported: vec!["RS256"],
    }))
}

async fn protected_resource_metadata_root(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<ProxyResourceMetadata>, HandlerError> {
    protected_resource_metadata_impl(state, None)
}

async fn protected_resource_metadata_with_path(
    Extension(state): Extension<Arc<AppState>>,
    Path(rest): Path<String>,
) -> Result<Json<ProxyResourceMetadata>, HandlerError> {
    protected_resource_metadata_impl(state, Some(rest))
}

fn protected_resource_metadata_impl(
    state: Arc<AppState>,
    rest: Option<String>,
) -> Result<Json<ProxyResourceMetadata>, HandlerError> {
    let base = state.config.server.public_url.trim_end_matches('/');
    let resource = if let Some(rest) = rest {
        format!("{}/{}", base, rest)
    } else {
        base.to_string()
    };
    let issuer = issuer_from_auth_url(&state.config.oauth.auth_url)?;
    Ok(Json(ProxyResourceMetadata {
        resource,
        authorization_servers: vec![issuer],
        scopes_supported: state.config.oauth.scopes.clone(),
    }))
}

async fn proxy_authorization_server_metadata(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<ProxyMetadata>, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    Ok(Json(proxy.metadata()))
}

async fn proxy_protected_resource_metadata_root(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<ProxyResourceMetadata>, HandlerError> {
    proxy_protected_resource_metadata_impl(state, None)
}

async fn proxy_protected_resource_metadata_with_path(
    Extension(state): Extension<Arc<AppState>>,
    Path(rest): Path<String>,
) -> Result<Json<ProxyResourceMetadata>, HandlerError> {
    proxy_protected_resource_metadata_impl(state, Some(rest))
}

fn proxy_protected_resource_metadata_impl(
    state: Arc<AppState>,
    rest: Option<String>,
) -> Result<Json<ProxyResourceMetadata>, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    let resource = if let Some(rest) = rest {
        format!("{}/{}", proxy.public_url, rest)
    } else {
        proxy.public_url.clone()
    };
    Ok(Json(proxy.protected_resource_metadata(resource)))
}

async fn proxy_openid_configuration(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<impl IntoResponse, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    let body = Json(proxy.openid_configuration());
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    Ok(response)
}

async fn proxy_register_client(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<ClientRegistrationRequest>,
) -> Result<Json<ClientRegistrationResponse>, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    let response = proxy
        .register_client(request)
        .map_err(|err| HandlerError::new(StatusCode::BAD_REQUEST, err.to_string(), Some(err)))?;
    Ok(Json(response))
}

async fn proxy_authorize(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<AuthorizationParams>,
) -> Result<impl IntoResponse, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    let url = proxy
        .start_authorization(&params)
        .map_err(|err| HandlerError::new(StatusCode::BAD_REQUEST, err.to_string(), Some(err)))?;
    Ok(Redirect::to(&url))
}

#[derive(Debug, Deserialize)]
struct ProxyCallbackQuery {
    state: String,
    code: String,
}

async fn proxy_callback(
    Extension(state): Extension<Arc<AppState>>,
    Query(query): Query<ProxyCallbackQuery>,
) -> Result<impl IntoResponse, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    let result = proxy
        .handle_callback(&query.state, &query.code)
        .map_err(|err| HandlerError::new(StatusCode::BAD_REQUEST, err.to_string(), Some(err)))?;

    let mut redirect_url = reqwest::Url::parse(&result.redirect_uri).map_err(|err| {
        HandlerError::new(
            StatusCode::BAD_REQUEST,
            "invalid redirect_uri",
            Some(err.into()),
        )
    })?;
    redirect_url
        .query_pairs_mut()
        .append_pair("code", &result.proxy_code);
    if let Some(state) = result.original_state {
        redirect_url.query_pairs_mut().append_pair("state", &state);
    }

    Ok(Redirect::to(redirect_url.as_str()))
}

async fn proxy_token(
    Extension(state): Extension<Arc<AppState>>,
    Form(form): Form<TokenRequest>,
) -> Result<Json<serde_json::Value>, HandlerError> {
    let proxy = require_proxy_state(&state)?;
    let response = proxy
        .exchange_code(&form)
        .await
        .map_err(|err| HandlerError::new(StatusCode::BAD_REQUEST, err.to_string(), Some(err)))?;
    Ok(Json(response.raw))
}

async fn openid_configuration(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<impl IntoResponse, HandlerError> {
    let issuer = issuer_from_auth_url(&state.config.oauth.auth_url)?;
    let jwks_uri = format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/'));
    let body = Json(ProxyOpenIdConfiguration {
        issuer,
        authorization_endpoint: state.config.oauth.auth_url.clone(),
        token_endpoint: state.config.oauth.token_url.clone(),
        jwks_uri,
        response_types_supported: vec!["code"],
        grant_types_supported: vec!["authorization_code"],
        code_challenge_methods_supported: vec!["S256"],
        scopes_supported: state.config.oauth.scopes.clone(),
        token_endpoint_auth_methods_supported: vec!["client_secret_post"],
        subject_types_supported: vec!["public"],
        id_token_signing_alg_values_supported: vec!["RS256"],
    });
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    Ok(response)
}

#[derive(Debug)]
pub struct HandlerError {
    code: StatusCode,
    message: String,
    source: Option<anyhow::Error>,
}

impl HandlerError {
    fn new(code: StatusCode, message: impl Into<String>, source: Option<anyhow::Error>) -> Self {
        Self {
            code,
            message: message.into(),
            source,
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message, None)
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> axum::response::Response {
        if let Some(source) = &self.source {
            tracing::error!(error = ?source, "handler error: {}", self.message);
        } else {
            tracing::warn!("handler error: {}", self.message);
        }

        let body = Json(json!({
            "error": self.message,
        }));
        (self.code, body).into_response()
    }
}

impl<E> From<E> for HandlerError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        let err = value.into();
        HandlerError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
            Some(err),
        )
    }
}

trait RequestUserId {
    fn user_id(&self) -> Option<&str>;
}

impl RequestUserId for ToolRequest {
    fn user_id(&self) -> Option<&str> {
        match self {
            ToolRequest::List { user_id, .. }
            | ToolRequest::Get { user_id, .. }
            | ToolRequest::Create { user_id, .. }
            | ToolRequest::Update { user_id, .. } => Some(user_id.as_str()),
        }
    }
}
