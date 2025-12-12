//! Secret Store
//!
//! Manages secrets with Docker Desktop + .env fallback

use crate::{GatewayError, Result};
use dashmap::DashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::{info, warn};

/// Secret Store
///
/// Provides secrets from multiple sources with fallback
pub struct SecretStore {
    secrets: Arc<DashMap<String, String>>,
    sources: Vec<String>,
}

impl SecretStore {
    /// Create a new secret store
    pub async fn new(sources: &[String]) -> Result<Self> {
        let store = Self {
            secrets: Arc::new(DashMap::new()),
            sources: sources.to_vec(),
        };

        store.load_secrets().await?;
        Ok(store)
    }

    /// Load secrets from all sources
    async fn load_secrets(&self) -> Result<()> {
        for source in &self.sources {
            match source.as_str() {
                "docker-desktop" => {
                    if let Err(e) = self.load_docker_desktop_secrets().await {
                        warn!("Failed to load Docker Desktop secrets: {}", e);
                    }
                }
                path if path.ends_with(".env") => {
                    if let Err(e) = self.load_env_file(Path::new(path)).await {
                        warn!("Failed to load .env file from {}: {}", path, e);
                    }
                }
                _ => {
                    warn!("Unknown secret source: {}", source);
                }
            }
        }

        info!("Loaded {} secrets", self.secrets.len());
        Ok(())
    }

    /// Load secrets from Docker Desktop
    async fn load_docker_desktop_secrets(&self) -> Result<()> {
        // Docker Desktop stores secrets in different locations based on OS
        #[cfg(target_os = "macos")]
        let secret_path = dirs::home_dir()
            .ok_or_else(|| GatewayError::Secret("Home directory not found".to_string()))?
            .join("Library/Group Containers/group.com.docker/secrets");

        #[cfg(target_os = "linux")]
        let secret_path = dirs::home_dir()
            .ok_or_else(|| GatewayError::Secret("Home directory not found".to_string()))?
            .join(".docker/secrets");

        #[cfg(target_os = "windows")]
        let secret_path = dirs::config_dir()
            .ok_or_else(|| GatewayError::Secret("Config directory not found".to_string()))?
            .join("Docker/secrets");

        if !secret_path.exists() {
            return Err(GatewayError::Secret(format!(
                "Docker Desktop secrets path not found: {:?}",
                secret_path
            )));
        }

        // Read all secret files
        let mut entries = fs::read_dir(&secret_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| GatewayError::Secret("Invalid secret name".to_string()))?;

                let value = fs::read_to_string(entry.path()).await?;
                self.secrets.insert(name, value.trim().to_string());
            }
        }

        info!("Loaded Docker Desktop secrets from {:?}", secret_path);
        Ok(())
    }

    /// Load secrets from .env file
    async fn load_env_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(GatewayError::Secret(format!(
                ".env file not found: {:?}",
                path
            )));
        }

        let content = fs::read_to_string(path).await?;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse KEY=VALUE
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                self.secrets.insert(key, value);
            }
        }

        info!("Loaded secrets from {:?}", path);
        Ok(())
    }

    /// Get a secret by key
    pub fn get_secret(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(|v| v.value().clone())
    }

    /// Set a secret
    pub fn set_secret(&self, key: String, value: String) {
        self.secrets.insert(key, value);
    }

    /// Delete a secret
    pub fn delete_secret(&self, key: &str) -> Option<String> {
        self.secrets.remove(key).map(|(_, v)| v)
    }

    /// Get all secret keys
    pub fn list_secrets(&self) -> Vec<String> {
        self.secrets
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get secret count
    pub fn secret_count(&self) -> usize {
        self.secrets.len()
    }

    /// Clear all secrets
    pub fn clear(&self) {
        self.secrets.clear();
    }

    /// Reload secrets from all sources
    pub async fn reload(&self) -> Result<()> {
        self.clear();
        self.load_secrets().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_load_env_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "API_KEY=test-key-123").unwrap();
        writeln!(temp_file, "SECRET_TOKEN=secret-value").unwrap();
        writeln!(temp_file, "# Comment line").unwrap();
        writeln!(temp_file, "").unwrap();
        writeln!(temp_file, "QUOTED=\"quoted value\"").unwrap();

        let store = SecretStore::new(&[]).await.unwrap();
        store.load_env_file(temp_file.path()).await.unwrap();

        assert_eq!(
            store.get_secret("API_KEY"),
            Some("test-key-123".to_string())
        );
        assert_eq!(
            store.get_secret("SECRET_TOKEN"),
            Some("secret-value".to_string())
        );
        assert_eq!(store.get_secret("QUOTED"), Some("quoted value".to_string()));
        assert!(store.get_secret("Comment").is_none());
    }

    #[tokio::test]
    async fn test_set_delete_secret() {
        let store = SecretStore::new(&[]).await.unwrap();

        store.set_secret("test_key".to_string(), "test_value".to_string());
        assert_eq!(store.get_secret("test_key"), Some("test_value".to_string()));

        let deleted = store.delete_secret("test_key");
        assert_eq!(deleted, Some("test_value".to_string()));
        assert!(store.get_secret("test_key").is_none());
    }
}
