use anyhow::Result;
use axum::Router;
use mcp_google_calendar::config::AppConfig;
use mcp_google_calendar::handlers::build_router;
use mcp_google_calendar::mcp::service_factory;
use mcp_google_calendar::oauth::storage::{FileTokenStorage, InMemoryTokenStorage, TokenStorage};
use mcp_google_calendar::AppState;
use rmcp::transport::sse_server::{SseServer, SseServerConfig};
use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = AppConfig::load()?;
    let storage: Arc<dyn TokenStorage> = if config.security.use_in_memory {
        Arc::new(InMemoryTokenStorage::new())
    } else {
        Arc::new(FileTokenStorage::new(
            &config.security.token_store_path,
            config.security.encrypt_tokens,
        )?)
    };

    let state = Arc::new(AppState::new(config, storage)?);
    let bind_address = state.config.server.bind_address.clone();

    let sse_config = SseServerConfig {
        bind: bind_address.parse()?,
        sse_path: "/sse".to_string(),
        post_path: "/message".to_string(),
        ct: CancellationToken::new(),
        sse_keep_alive: None,
    };

    let (sse_server, sse_router) = SseServer::new(sse_config);
    let service_factory = service_factory(state.clone());
    let sse_token = sse_server.with_service(service_factory);

    let router = Router::new()
        .merge(build_router(state.clone()))
        .nest("/mcp", sse_router);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;

    tracing::info!(
        bind_address = %bind_address,
        public_url = %state.config.server.public_url,
        sse_path = "/mcp/sse",
        "starting server"
    );

    let shutdown_token = sse_token.clone();
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        tokio::select! {
            _ = shutdown_signal() => {
                shutdown_token.cancel();
            }
            _ = shutdown_token.cancelled() => {}
        }
    });

    server.await?;
    sse_token.cancel();
    Ok(())
}

fn init_tracing() {
    if tracing::dispatcher::has_been_set() {
        return;
    }

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
