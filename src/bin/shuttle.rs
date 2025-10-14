use std::sync::Arc;

use axum::{Extension, Router};
use mcp_google_calendar::{
    config::AppConfig,
    handlers::build_router,
    mcp::service_factory,
    oauth::storage::{InMemoryTokenStorage, TokenStorage},
    AppState,
};
use rmcp::transport::sse_server::{SseServer, SseServerConfig};
use shuttle_runtime::SecretStore;
use tokio_util::sync::CancellationToken;

#[shuttle_runtime::main]
async fn shuttle(#[shuttle_runtime::Secrets] secrets: SecretStore) -> shuttle_axum::ShuttleAxum {
    if let Some(client_id) = secrets.get("OAUTH_CLIENT_ID") {
        std::env::set_var("APP__OAUTH__CLIENT_ID", client_id);
    }
    if let Some(client_secret) = secrets.get("OAUTH_CLIENT_SECRET") {
        std::env::set_var("APP__OAUTH__CLIENT_SECRET", client_secret);
    }
    if let Some(redirect_uri) = secrets.get("OAUTH_REDIRECT_URI") {
        std::env::set_var("APP__OAUTH__REDIRECT_URI", redirect_uri);
    }

    std::env::set_var(
        "APP__PROXY__ENABLED",
        secrets
            .get("PROXY_ENABLED")
            .unwrap_or_else(|| "true".to_owned()),
    );
    std::env::set_var(
        "APP__SECURITY__USE_IN_MEMORY",
        secrets
            .get("SECURITY__USE_IN_MEMORY")
            .unwrap_or_else(|| "true".to_owned()),
    );

    std::env::set_var(
        "APP__SERVER__BIND_ADDRESS",
        secrets
            .get("SERVER__BIND_ADDRESS")
            .unwrap_or_else(|| "0.0.0.0:8000".to_owned()),
    );
    if let Some(public_url) = secrets.get("SERVER__PUBLIC_URL") {
        std::env::set_var("APP__SERVER__PUBLIC_URL", public_url);
    }

    let config = AppConfig::load().expect("load app config");
    let storage: Arc<dyn TokenStorage> = Arc::new(InMemoryTokenStorage::new());
    let state = Arc::new(AppState::new(config, storage).expect("initialize app state"));

    let bind_address = state
        .config
        .server
        .bind_address
        .parse()
        .expect("invalid bind address");
    let sse_config = SseServerConfig {
        bind: bind_address,
        sse_path: "/".to_owned(),
        post_path: "/message".to_owned(),
        ct: CancellationToken::new(),
        sse_keep_alive: None,
    };

    let (sse_server, sse_router) = SseServer::new(sse_config);
    let sse_token = sse_server.with_service(service_factory(state.clone()));

    let router: Router = build_router(state.clone())
        .nest("/mcp", sse_router)
        .layer(Extension(sse_token));

    Ok(router.into())
}
