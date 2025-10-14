pub mod storage;

use crate::config::OAuthConfig;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use oauth2::basic::{BasicClient, BasicTokenType};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl,
    RefreshToken, Scope, StandardTokenResponse, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizationContext {
    pub authorize_url: Url,
    pub csrf_state: String,
    pub pkce_verifier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    pub refresh_token: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
    pub token_type: String,
}

impl TokenInfo {
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => Utc::now() + Duration::seconds(30) >= expiry,
            None => false,
        }
    }
}

pub struct OAuthClient {
    client: BasicClient,
    scopes: Vec<Scope>,
}

impl OAuthClient {
    pub fn from_config(config: &OAuthConfig) -> Result<Self> {
        let client_id = oauth2::ClientId::new(config.client_id.clone());
        let client_secret = oauth2::ClientSecret::new(config.client_secret.clone());
        let auth_url =
            AuthUrl::new(config.auth_url.clone()).map_err(|e| anyhow!("invalid auth url: {e}"))?;
        let token_url = TokenUrl::new(config.token_url.clone())
            .map_err(|e| anyhow!("invalid token url: {e}"))?;

        let mut client =
            BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url));

        // Redirect URL will be supplied per request because multi-client setups may vary.
        let scopes = config
            .scopes
            .iter()
            .cloned()
            .map(Scope::new)
            .collect::<Vec<_>>();

        client = client.set_auth_type(oauth2::AuthType::RequestBody);

        Ok(Self { client, scopes })
    }

    pub fn authorize_url(&self, redirect_uri: &str) -> Result<AuthorizationContext> {
        let redirect = RedirectUrl::new(redirect_uri.to_owned())
            .map_err(|err| anyhow!("invalid redirect url: {err}"))?;

        let client = self.client.clone().set_redirect_uri(redirect);
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge)
            .add_extra_param("prompt", "select_account");

        for scope in &self.scopes {
            request = request.add_scope(scope.clone());
        }

        let (authorize_url, csrf_state) = request.url();

        Ok(AuthorizationContext {
            authorize_url,
            csrf_state: csrf_state.secret().to_owned(),
            pkce_verifier: pkce_verifier.secret().to_owned(),
        })
    }

    pub async fn exchange_code(
        &self,
        redirect_uri: &str,
        authorization_code: &str,
        pkce_verifier: &str,
    ) -> Result<TokenInfo> {
        let redirect = RedirectUrl::new(redirect_uri.to_owned())
            .map_err(|err| anyhow!("invalid redirect url: {err}"))?;

        let client = self.client.clone().set_redirect_uri(redirect);
        let token = client
            .exchange_code(AuthorizationCode::new(authorization_code.to_owned()))
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.to_owned()))
            .request_async(async_http_client)
            .await?;

        Self::map_token_response(token)
    }

    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<TokenInfo> {
        let response = self
            .client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.to_owned()))
            .request_async(async_http_client)
            .await?;

        Self::map_token_response(response)
    }

    fn map_token_response(
        response: StandardTokenResponse<oauth2::EmptyExtraTokenFields, BasicTokenType>,
    ) -> Result<TokenInfo> {
        let access_token = response.access_token().secret().to_owned();
        let refresh_token = response
            .refresh_token()
            .map(|token| token.secret().to_owned());
        let expires_at = response
            .expires_in()
            .map(|duration| Utc::now() + Duration::from_std(duration).unwrap_or_default());
        let scope = response.scopes().map(|scopes| {
            scopes
                .iter()
                .map(|s| s.as_ref())
                .collect::<Vec<_>>()
                .join(" ")
        });

        Ok(TokenInfo {
            access_token,
            refresh_token,
            expires_at,
            scope,
            token_type: response.token_type().as_ref().to_owned(),
        })
    }
}
