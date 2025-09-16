use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::PluginManifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistry {
    pub plugins: HashMap<String, RegistryEntry>,
    pub sources: Vec<RegistrySource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub manifest: PluginManifest,
    pub download_url: String,
    pub checksum: String,
    pub verified: bool,
    pub downloads: u64,
    pub rating: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySource {
    pub name: String,
    pub url: String,
    pub trusted: bool,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            sources: vec![
                RegistrySource {
                    name: "official".to_string(),
                    url: "https://registry.bolt.dev/plugins".to_string(),
                    trusted: true,
                },
                RegistrySource {
                    name: "community".to_string(),
                    url: "https://community.bolt.dev/plugins".to_string(),
                    trusted: false,
                },
            ],
        }
    }

    pub async fn sync(&mut self) -> Result<()> {
        for source in &self.sources {
            self.sync_source(source).await?;
        }
        Ok(())
    }

    pub async fn search(&self, query: &str) -> Vec<&RegistryEntry> {
        self.plugins
            .values()
            .filter(|entry| {
                entry.manifest.name.contains(query) ||
                entry.manifest.description.contains(query)
            })
            .collect()
    }

    pub async fn install(&self, plugin_name: &str, install_path: &PathBuf) -> Result<()> {
        let entry = self.plugins.get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {}", plugin_name))?;

        self.download_plugin(entry, install_path).await?;
        self.verify_plugin(entry, install_path).await?;

        Ok(())
    }

    pub async fn uninstall(&self, plugin_name: &str, install_path: &PathBuf) -> Result<()> {
        let plugin_path = install_path.join(plugin_name);
        if plugin_path.exists() {
            tokio::fs::remove_dir_all(plugin_path).await?;
        }
        Ok(())
    }

    pub fn list_by_type(&self, plugin_type: &super::PluginType) -> Vec<&RegistryEntry> {
        self.plugins
            .values()
            .filter(|entry| std::mem::discriminant(&entry.manifest.plugin_type) == std::mem::discriminant(plugin_type))
            .collect()
    }

    async fn sync_source(&mut self, source: &RegistrySource) -> Result<()> {
        let url = format!("{}/index.json", source.url);
        let response = reqwest::get(&url).await?;
        let registry_data: HashMap<String, RegistryEntry> = response.json().await?;

        for (name, mut entry) in registry_data {
            if !source.trusted {
                entry.verified = false;
            }
            self.plugins.insert(name, entry);
        }

        Ok(())
    }

    async fn download_plugin(&self, entry: &RegistryEntry, install_path: &PathBuf) -> Result<()> {
        let plugin_dir = install_path.join(&entry.manifest.name);
        tokio::fs::create_dir_all(&plugin_dir).await?;

        let response = reqwest::get(&entry.download_url).await?;
        let bytes = response.bytes().await?;

        let archive_path = plugin_dir.join("plugin.tar.gz");
        tokio::fs::write(&archive_path, bytes).await?;

        self.extract_plugin(&archive_path, &plugin_dir).await?;

        Ok(())
    }

    async fn verify_plugin(&self, entry: &RegistryEntry, install_path: &PathBuf) -> Result<()> {
        let plugin_dir = install_path.join(&entry.manifest.name);
        let archive_path = plugin_dir.join("plugin.tar.gz");

        let file_bytes = tokio::fs::read(&archive_path).await?;
        let calculated_checksum = sha2::Sha256::digest(&file_bytes);
        let calculated_hex = hex::encode(calculated_checksum);

        if calculated_hex != entry.checksum {
            return Err(anyhow::anyhow!("Plugin checksum verification failed"));
        }

        Ok(())
    }

    async fn extract_plugin(&self, archive_path: &PathBuf, destination: &PathBuf) -> Result<()> {
        let file = std::fs::File::open(archive_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(destination)?;
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}