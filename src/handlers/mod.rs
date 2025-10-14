use crate::mcp::{HttpMcpServer, ToolRequest, ToolResponse};
use crate::{AppState, AuthorizationSession};
use anyhow::Context;
use axum::extract::{Path, Query};
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, WWW_AUTHENTICATE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::{routing::get, Extension, Router};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/callback", get(callback))
        .route("/mcp/tool", axum::routing::post(handle_tool))
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
        )
        .layer(Extension(state))
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
    Json(request): Json<ToolRequest>,
) -> Result<impl IntoResponse, HandlerError> {
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

            let metadata_url = protected_resource_metadata_url(&state.config.server.public_url);
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

#[derive(Debug, Serialize)]
struct AuthorizationServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    response_types_supported: Vec<&'static str>,
    grant_types_supported: Vec<&'static str>,
    code_challenge_methods_supported: Vec<&'static str>,
    scopes_supported: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: Vec<String>,
    scopes_supported: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OpenIdConfiguration {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    response_types_supported: Vec<&'static str>,
    grant_types_supported: Vec<&'static str>,
    code_challenge_methods_supported: Vec<&'static str>,
    scopes_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<&'static str>,
    subject_types_supported: Vec<&'static str>,
    id_token_signing_alg_values_supported: Vec<&'static str>,
}

async fn authorization_server_metadata(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<AuthorizationServerMetadata>, HandlerError> {
    let issuer = issuer_from_auth_url(&state.config.oauth.auth_url)?;
    Ok(Json(AuthorizationServerMetadata {
        issuer,
        authorization_endpoint: state.config.oauth.auth_url.clone(),
        token_endpoint: state.config.oauth.token_url.clone(),
        response_types_supported: vec!["code"],
        grant_types_supported: vec!["authorization_code"],
        code_challenge_methods_supported: vec!["S256"],
        scopes_supported: state.config.oauth.scopes.clone(),
    }))
}

async fn protected_resource_metadata_root(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<ProtectedResourceMetadata>, HandlerError> {
    protected_resource_metadata_impl(state, None)
}

async fn protected_resource_metadata_with_path(
    Extension(state): Extension<Arc<AppState>>,
    Path(rest): Path<String>,
) -> Result<Json<ProtectedResourceMetadata>, HandlerError> {
    protected_resource_metadata_impl(state, Some(rest))
}

fn protected_resource_metadata_impl(
    state: Arc<AppState>,
    rest: Option<String>,
) -> Result<Json<ProtectedResourceMetadata>, HandlerError> {
    let base = state.config.server.public_url.trim_end_matches('/');
    let resource = if let Some(rest) = rest {
        format!("{}/{}", base, rest)
    } else {
        base.to_string()
    };
    let issuer = issuer_from_auth_url(&state.config.oauth.auth_url)?;
    Ok(Json(ProtectedResourceMetadata {
        resource,
        authorization_servers: vec![issuer],
        scopes_supported: state.config.oauth.scopes.clone(),
    }))
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

async fn openid_configuration(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<impl IntoResponse, HandlerError> {
    let issuer = issuer_from_auth_url(&state.config.oauth.auth_url)?;
    let jwks_uri = format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/'));
    let body = Json(OpenIdConfiguration {
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
