//! Model caching and HuggingFace Hub integration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Model cache manager
pub struct ModelCache {
    cache_dir: PathBuf,
    models: HashMap<String, CachedModel>,
    dedup_store: ContentAddressableStore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedModel {
    pub model_id: String,
    pub source: ModelSource,
    pub files: Vec<CachedFile>,
    pub total_size_bytes: u64,
    pub downloaded_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelSource {
    HuggingFace { repo_id: String },
    Local { path: PathBuf },
    Custom { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    pub path: PathBuf,
    pub content_hash: String, // SHA256
    pub size_bytes: u64,
    pub is_deduplicated: bool,
}

/// Content-addressable storage for deduplication
struct ContentAddressableStore {
    store_dir: PathBuf,
    index: HashMap<String, PathBuf>,
}

impl ModelCache {
    /// Create a new model cache
    pub async fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_dir()?;
        Self::new_in(cache_dir).await
    }

    async fn new_in(cache_dir: PathBuf) -> Result<Self> {
        tokio::fs::create_dir_all(&cache_dir).await?;

        let dedup_dir = cache_dir.join(".dedup");
        tokio::fs::create_dir_all(&dedup_dir).await?;

        info!("📦 Model cache initialized: {}", cache_dir.display());

        Ok(Self {
            cache_dir,
            models: HashMap::new(),
            dedup_store: ContentAddressableStore::new(dedup_dir),
        })
    }

    /// Pull model from HuggingFace Hub
    pub async fn pull_huggingface(&mut self, repo_id: &str) -> Result<PathBuf> {
        info!("📥 Pulling model from HuggingFace: {}", repo_id);

        // Check if already cached
        if let Some(model) = self.models.get(repo_id) {
            info!("✅ Model already cached, skipping download");
            return Ok(model.files[0].path.parent().unwrap().to_path_buf());
        }

        // Download model files
        let model_dir = self.cache_dir.join(repo_id.replace('/', "--"));
        tokio::fs::create_dir_all(&model_dir).await?;

        let files = self.download_huggingface_files(repo_id, &model_dir).await?;

        // Deduplicate common files
        let mut dedup_files = Vec::new();
        for file in files {
            let dedup_file = self.dedup_store.add_file(&file).await?;
            dedup_files.push(dedup_file);
        }

        let total_size: u64 = dedup_files.iter().map(|f| f.size_bytes).sum();

        let cached_model = CachedModel {
            model_id: repo_id.to_string(),
            source: ModelSource::HuggingFace {
                repo_id: repo_id.to_string(),
            },
            files: dedup_files,
            total_size_bytes: total_size,
            downloaded_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
        };

        self.models.insert(repo_id.to_string(), cached_model);

        info!(
            "✅ Model downloaded and cached: {} ({} MB)",
            repo_id,
            total_size / 1024 / 1024
        );

        Ok(model_dir)
    }

    async fn download_huggingface_files(
        &self,
        repo_id: &str,
        model_dir: &Path,
    ) -> Result<Vec<CachedFile>> {
        // Use huggingface_hub Python library or API
        // For now, simulate download

        info!("   Downloading model files for {}...", repo_id);

        // Common files in HuggingFace models
        let files = vec![
            "config.json",
            "tokenizer_config.json",
            "tokenizer.json",
            "special_tokens_map.json",
            "pytorch_model.bin", // or model.safetensors
        ];

        let mut cached_files = Vec::new();

        for filename in files {
            let file_path = model_dir.join(filename);

            // Simulate download
            debug!("   • Downloading {}", filename);
            tokio::fs::write(&file_path, b"mock content").await?;

            let metadata = tokio::fs::metadata(&file_path).await?;
            let hash = self.compute_file_hash(&file_path).await?;

            cached_files.push(CachedFile {
                path: file_path,
                content_hash: hash,
                size_bytes: metadata.len(),
                is_deduplicated: false,
            });
        }

        Ok(cached_files)
    }

    async fn compute_file_hash(&self, path: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};

        let content = tokio::fs::read(path).await?;
        let hash = Sha256::digest(&content);
        Ok(format!("{:x}", hash))
    }

    /// List cached models
    pub fn list_models(&self) -> Vec<&CachedModel> {
        self.models.values().collect()
    }

    /// Get cached model path
    pub fn get_model_path(&self, model_id: &str) -> Option<PathBuf> {
        self.models.get(model_id).and_then(|model| {
            model
                .files
                .first()
                .map(|f| f.path.parent().unwrap().to_path_buf())
        })
    }

    /// Prune unused models
    pub async fn prune(&mut self, keep_recent_days: u64) -> Result<Vec<String>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(keep_recent_days as i64);
        let mut pruned = Vec::new();

        self.models.retain(|id, model| {
            if model.last_accessed < cutoff {
                info!("🗑️  Pruning unused model: {}", id);
                pruned.push(id.clone());
                false
            } else {
                true
            }
        });

        // Delete files
        for model_id in &pruned {
            let model_dir = self.cache_dir.join(model_id.replace('/', "--"));
            if model_dir.exists() {
                tokio::fs::remove_dir_all(&model_dir).await?;
            }
        }

        info!("✅ Pruned {} unused models", pruned.len());
        Ok(pruned)
    }

    fn get_cache_dir() -> Result<PathBuf> {
        // Use XDG_CACHE_HOME or ~/.cache/bolt/models
        let cache_dir = if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
            PathBuf::from(xdg_cache).join("bolt/models")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".cache/bolt/models")
        } else {
            PathBuf::from("/var/cache/bolt/models")
        };

        Ok(cache_dir)
    }
}

impl ContentAddressableStore {
    fn new(store_dir: PathBuf) -> Self {
        Self {
            store_dir,
            index: HashMap::new(),
        }
    }

    async fn add_file(&mut self, file: &CachedFile) -> Result<CachedFile> {
        // Check if file already exists by hash
        if let Some(existing_path) = self.index.get(&file.content_hash) {
            // File already exists, create hardlink
            debug!("   • Deduplicating: {}", file.path.display());

            tokio::fs::hard_link(existing_path, &file.path).await?;

            return Ok(CachedFile {
                is_deduplicated: true,
                ..file.clone()
            });
        }

        // Store file in dedup store
        let store_path = self.store_dir.join(&file.content_hash);
        tokio::fs::copy(&file.path, &store_path).await?;

        self.index.insert(file.content_hash.clone(), store_path);

        Ok(file.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_cache_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = ModelCache::new_in(temp_dir.path().join("models"))
            .await
            .unwrap();
        assert!(cache.cache_dir.exists());
    }
}
