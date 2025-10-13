use super::TokenInfo;
use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::task;

#[async_trait]
pub trait TokenStorage: Send + Sync + 'static {
    async fn fetch(&self, user_id: &str) -> Result<Option<TokenInfo>>;
    async fn persist(&self, user_id: &str, token: &TokenInfo) -> Result<()>;
    async fn revoke(&self, user_id: &str) -> Result<()>;
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenRecord {
    #[serde(flatten)]
    token: TokenInfo,
}

#[derive(Debug)]
pub struct FileTokenStorage {
    path: PathBuf,
    encrypt: bool,
    cache: RwLock<HashMap<String, TokenInfo>>,
}

impl FileTokenStorage {
    pub fn new(path: impl AsRef<Path>, encrypt: bool) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let cache = if path.exists() {
            let data = fs::read_to_string(&path)?;
            if data.trim().is_empty() {
                HashMap::new()
            } else {
                serde_json::from_str::<HashMap<String, TokenRecord>>(&data)?
                    .into_iter()
                    .map(|(k, v)| (k, v.token))
                    .collect()
            }
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            HashMap::new()
        };

        Ok(Self {
            path,
            encrypt,
            cache: RwLock::new(cache),
        })
    }

    fn serialize(tokens: &HashMap<String, TokenInfo>) -> Result<String> {
        let wrapper = tokens
            .iter()
            .map(|(k, v)| (k.clone(), TokenRecord { token: v.clone() }))
            .collect::<HashMap<_, _>>();
        let data = serde_json::to_string_pretty(&wrapper)?;
        Ok(data)
    }

    fn write_to_disk(path: PathBuf, body: String) -> Result<()> {
        fs::write(path, body)?;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryTokenStorage {
    cache: RwLock<HashMap<String, TokenInfo>>,
}

impl InMemoryTokenStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl TokenStorage for FileTokenStorage {
    async fn fetch(&self, user_id: &str) -> Result<Option<TokenInfo>> {
        let cache = self.cache.read();
        Ok(cache.get(user_id).cloned())
    }

    async fn persist(&self, user_id: &str, token: &TokenInfo) -> Result<()> {
        if self.encrypt {
            tracing::warn!("token encryption not yet implemented; storing plaintext");
        }

        {
            let mut cache = self.cache.write();
            cache.insert(user_id.to_owned(), token.clone());
        }

        let cache_snapshot = { self.cache.read().clone() };
        let path = self.path.clone();

        task::spawn_blocking(move || {
            let serialized = FileTokenStorage::serialize(&cache_snapshot)?;
            FileTokenStorage::write_to_disk(path, serialized)
        })
        .await?
    }

    async fn revoke(&self, user_id: &str) -> Result<()> {
        {
            let mut cache = self.cache.write();
            cache.remove(user_id);
        }

        let cache_snapshot = { self.cache.read().clone() };
        let path = self.path.clone();

        task::spawn_blocking(move || {
            let serialized = FileTokenStorage::serialize(&cache_snapshot)?;
            FileTokenStorage::write_to_disk(path, serialized)
        })
        .await??;

        Ok(())
    }
}

#[async_trait]
impl TokenStorage for InMemoryTokenStorage {
    async fn fetch(&self, user_id: &str) -> Result<Option<TokenInfo>> {
        Ok(self.cache.read().get(user_id).cloned())
    }

    async fn persist(&self, user_id: &str, token: &TokenInfo) -> Result<()> {
        self.cache.write().insert(user_id.to_owned(), token.clone());
        Ok(())
    }

    async fn revoke(&self, user_id: &str) -> Result<()> {
        self.cache.write().remove(user_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::fs;

    #[tokio::test]
    async fn persist_and_fetch_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokens.json");
        let storage = FileTokenStorage::new(&path, false).unwrap();

        let token = TokenInfo {
            access_token: "access".into(),
            refresh_token: Some("refresh".into()),
            expires_at: Some(Utc::now() + Duration::minutes(5)),
            scope: Some("scope".into()),
            token_type: "Bearer".into(),
        };

        storage.persist("user", &token).await.unwrap();
        let loaded = storage.fetch("user").await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().access_token, "access");

        storage.revoke("user").await.unwrap();
        assert!(storage.fetch("user").await.unwrap().is_none());

        assert!(fs::metadata(path).is_ok());
    }
}
