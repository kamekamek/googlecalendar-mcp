use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    pub oauth: OAuthConfig,
    #[serde(default)]
    pub google: GoogleConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "ServerConfig::default_bind_address")]
    pub bind_address: String,
    #[serde(default = "ServerConfig::default_public_url")]
    pub public_url: String,
}

impl ServerConfig {
    fn default_bind_address() -> String {
        "127.0.0.1:8080".to_owned()
    }

    fn default_public_url() -> String {
        "http://localhost:8080".to_owned()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: Self::default_bind_address(),
            public_url: Self::default_public_url(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "OAuthConfig::default_auth_url")]
    pub auth_url: String,
    #[serde(default = "OAuthConfig::default_token_url")]
    pub token_url: String,
    #[serde(default = "OAuthConfig::default_redirect_uri")]
    pub redirect_uri: String,
    #[serde(default = "OAuthConfig::default_scopes")]
    pub scopes: Vec<String>,
}

impl OAuthConfig {
    fn default_auth_url() -> String {
        "https://accounts.google.com/o/oauth2/v2/auth".to_owned()
    }

    fn default_token_url() -> String {
        "https://oauth2.googleapis.com/token".to_owned()
    }

    fn default_redirect_uri() -> String {
        "http://localhost:8080/oauth/callback".to_owned()
    }

    fn default_scopes() -> Vec<String> {
        vec!["https://www.googleapis.com/auth/calendar".to_owned()]
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleConfig {
    #[serde(default = "GoogleConfig::default_api_base")]
    pub api_base: String,
    #[serde(default)]
    pub calendar_id: Option<String>,
}

impl GoogleConfig {
    fn default_api_base() -> String {
        "https://www.googleapis.com/calendar/v3".to_owned()
    }
}

impl Default for GoogleConfig {
    fn default() -> Self {
        Self {
            api_base: Self::default_api_base(),
            calendar_id: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "SecurityConfig::default_token_store_path")]
    pub token_store_path: String,
    #[serde(default = "SecurityConfig::default_encrypt_tokens")]
    pub encrypt_tokens: bool,
    #[serde(default = "SecurityConfig::default_use_in_memory")]
    pub use_in_memory: bool,
}

impl SecurityConfig {
    fn default_token_store_path() -> String {
        "config/tokens.json".to_owned()
    }

    fn default_encrypt_tokens() -> bool {
        false
    }

    fn default_use_in_memory() -> bool {
        false
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            token_store_path: Self::default_token_store_path(),
            encrypt_tokens: Self::default_encrypt_tokens(),
            use_in_memory: Self::default_use_in_memory(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub redirect_path: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            redirect_path: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let builder = Config::builder()
            .add_source(File::with_name("config/config").required(false))
            .add_source(File::with_name("config/config.local").required(false))
            .add_source(Environment::with_prefix("APP").separator("__"));

        let cfg = builder.build()?;
        cfg.try_deserialize().map_err(|err: ConfigError| err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_set() {
        let cfg = AppConfig {
            server: ServerConfig {
                bind_address: ServerConfig::default_bind_address(),
                public_url: ServerConfig::default_public_url(),
            },
            oauth: OAuthConfig {
                client_id: "client".into(),
                client_secret: "secret".into(),
                auth_url: "https://example.com/auth".into(),
                token_url: "https://example.com/token".into(),
                redirect_uri: "http://localhost/oauth/callback".into(),
                scopes: OAuthConfig::default_scopes(),
            },
            google: GoogleConfig {
                api_base: GoogleConfig::default_api_base(),
                calendar_id: None,
            },
            security: SecurityConfig {
                token_store_path: SecurityConfig::default_token_store_path(),
                encrypt_tokens: SecurityConfig::default_encrypt_tokens(),
                use_in_memory: SecurityConfig::default_use_in_memory(),
            },
            proxy: ProxyConfig::default(),
        };

        assert_eq!(cfg.server.bind_address, "127.0.0.1:8080");
        assert_eq!(
            cfg.google.api_base,
            "https://www.googleapis.com/calendar/v3"
        );
        assert!(!cfg.security.encrypt_tokens);
        assert!(!cfg.security.use_in_memory);
        assert!(!cfg.proxy.enabled);
    }
}
