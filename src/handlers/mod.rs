use crate::mcp::{HttpMcpServer, ToolRequest, ToolResponse};
use crate::{AppState, AuthorizationSession};
use anyhow::Context;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::{routing::get, Extension, Router};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/callback", get(callback))
        .route("/mcp/tool", axum::routing::post(handle_tool))
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
            return Ok((StatusCode::UNAUTHORIZED, Json(response)));
        }
    }

    let server = HttpMcpServer::new(state);
    let response = server.handle_request(request).await;
    let status = match response.status {
        crate::mcp::ResponseStatus::Success => StatusCode::OK,
        crate::mcp::ResponseStatus::Error => StatusCode::BAD_REQUEST,
    };
    Ok((status, Json(response)))
}

fn cleanup_sessions(state: &Arc<AppState>) {
    let cutoff = Utc::now() - Duration::minutes(10);
    let mut sessions = state.auth_sessions.write();
    sessions.retain(|_, session| session.created_at > cutoff);
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
