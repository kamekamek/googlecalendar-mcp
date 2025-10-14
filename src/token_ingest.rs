use crate::{oauth::TokenInfo, AppState};
use axum::http::{header::AUTHORIZATION, HeaderMap};
use chrono::{DateTime, Duration, Utc};
use std::{sync::Arc, time::Duration as StdDuration};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BearerTokenError {
    #[error("authorization header must be valid UTF-8")]
    InvalidUtf8(#[from] axum::http::header::ToStrError),
    #[error("token storage error: {0}")]
    Storage(#[from] anyhow::Error),
}

pub async fn ingest_bearer_token_from_headers(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    user_id: &str,
) -> Result<Option<TokenInfo>, BearerTokenError> {
    let Some(raw_header) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };

    let header_value = raw_header.to_str().map_err(BearerTokenError::InvalidUtf8)?;
    let mut parts = header_value.splitn(2, ' ');
    let scheme = parts.next().unwrap_or_default();
    let token_part = parts.next().unwrap_or_default().trim();

    if !scheme.eq_ignore_ascii_case("Bearer") || token_part.is_empty() {
        return Ok(None);
    }

    {
        let revoked = state.revoked_tokens.read();
        if revoked
            .get(user_id)
            .map(|tokens| tokens.contains(token_part))
            .unwrap_or(false)
        {
            tracing::info!(user_id = %user_id, "ignoring revoked bearer token from headers");
            return Ok(None);
        }
    }

    let (refresh_token, refresh_present) = header_with_presence(
        headers,
        &["x-mcp-oauth-refresh-token", "x-oauth-refresh-token"],
    );
    let (scope, scope_present) =
        header_with_presence(headers, &["x-mcp-oauth-scope", "x-oauth-scope"]);
    let (expires_at, expires_present) = parse_expires_metadata(headers);
    let (token_type_header, _) =
        header_with_presence(headers, &["x-mcp-oauth-token-type", "x-oauth-token-type"]);
    let default_token_type = if scheme.eq_ignore_ascii_case("bearer") {
        "Bearer".to_owned()
    } else {
        scheme.to_owned()
    };
    let token_type = token_type_header.unwrap_or(default_token_type);

    let existing = state
        .token_storage
        .fetch(user_id)
        .await
        .map_err(BearerTokenError::Storage)?;
    let had_existing = existing.is_some();
    let mut token_info = existing.unwrap_or(TokenInfo {
        access_token: token_part.to_owned(),
        refresh_token: refresh_token.clone(),
        expires_at: expires_at.clone(),
        scope: scope.clone(),
        token_type: token_type.clone(),
    });

    let mut needs_persist = !had_existing;

    if token_info.access_token != token_part {
        token_info.access_token = token_part.to_owned();
        needs_persist = true;
    }

    if refresh_present && token_info.refresh_token != refresh_token {
        token_info.refresh_token = refresh_token.clone();
        needs_persist = true;
    }

    if scope_present && token_info.scope != scope {
        token_info.scope = scope.clone();
        needs_persist = true;
    }

    if expires_present && token_info.expires_at != expires_at {
        token_info.expires_at = expires_at.clone();
        needs_persist = true;
    }

    if token_info.token_type != token_type {
        token_info.token_type = token_type.clone();
        needs_persist = true;
    }

    if needs_persist {
        state
            .token_storage
            .persist(user_id, &token_info)
            .await
            .map_err(BearerTokenError::Storage)?;
        state.revoked_tokens.write().remove(user_id);
        if had_existing {
            tracing::info!(user_id = %user_id, "updated bearer token from headers");
        } else {
            tracing::info!(user_id = %user_id, "stored bearer token from headers");
        }
    }

    Ok(Some(token_info))
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
