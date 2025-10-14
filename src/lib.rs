pub mod config;
pub mod google_calendar;
pub mod handlers;
pub mod mcp;
pub mod oauth;
pub mod proxy;
pub mod token_ingest;

use anyhow::Result;
use config::AppConfig;
use google_calendar::GoogleCalendarClient;
use oauth::{storage::TokenStorage, AuthorizationContext, OAuthClient};
use parking_lot::RwLock;
use proxy::ProxyState;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{DateTime, Utc};

pub struct AppState {
    pub config: AppConfig,
    pub oauth_client: OAuthClient,
    pub google_calendar: GoogleCalendarClient,
    pub token_storage: Arc<dyn TokenStorage>,
    pub auth_sessions: Arc<RwLock<HashMap<String, AuthorizationSession>>>,
    pub revoked_tokens: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    pub proxy_state: Option<Arc<ProxyState>>,
}

impl AppState {
    pub fn new(config: AppConfig, storage: Arc<dyn TokenStorage>) -> Result<Self> {
        let oauth_client = OAuthClient::from_config(&config.oauth)?;
        let google_calendar = GoogleCalendarClient::new(config.google.api_base.clone())
            .with_default_calendar(config.google.calendar_id.clone());

        let proxy_state = if config.proxy.enabled {
            Some(Arc::new(ProxyState::new(&config)?))
        } else {
            None
        };

        Ok(Self {
            config,
            oauth_client,
            google_calendar,
            token_storage: storage,
            auth_sessions: Arc::new(RwLock::new(HashMap::new())),
            revoked_tokens: Arc::new(RwLock::new(HashMap::new())),
            proxy_state,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizationSession {
    pub user_id: String,
    pub state: AuthorizationContext,
    pub created_at: DateTime<Utc>,
}
