use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod community;
pub mod repository;
pub mod validation;

use crate::optimizations::OptimizationProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRegistry {
    pub profiles: HashMap<String, ProfileEntry>,
    pub repositories: Vec<ProfileRepository>,
    pub user_profiles: HashMap<String, OptimizationProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub profile: OptimizationProfile,
    pub metadata: ProfileMetadata,
    pub source: ProfileSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub author: String,
    pub version: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub downloads: u64,
    pub rating: f32,
    pub verified: bool,
    pub tags: Vec<String>,
    pub compatible_games: Vec<String>,
    pub min_requirements: SystemRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRequirements {
    pub min_cpu_cores: Option<u32>,
    pub min_memory_gb: Option<u32>,
    pub required_gpu_vendor: Option<crate::plugins::GpuVendor>,
    pub min_gpu_memory_gb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRepository {
    pub name: String,
    pub url: String,
    pub trusted: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProfileSource {
    Official,
    Community { repository: String },
    User,
    Local,
}

pub struct ProfileManager {
    registry: Arc<RwLock<ProfileRegistry>>,
    cache_dir: PathBuf,
    user_profiles_dir: PathBuf,
}

impl ProfileManager {
    pub fn new(cache_dir: PathBuf, user_profiles_dir: PathBuf) -> Self {
        Self {
            registry: Arc::new(RwLock::new(ProfileRegistry::new())),
            cache_dir,
            user_profiles_dir,
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        self.load_user_profiles().await?;
        self.sync_repositories().await?;
        Ok(())
    }

    pub async fn search_profiles(&self, query: &str, tags: &[String]) -> Vec<ProfileEntry> {
        let registry = self.registry.read().await;

        registry.profiles
            .values()
            .filter(|entry| {
                let name_match = entry.profile.name.to_lowercase().contains(&query.to_lowercase());
                let desc_match = entry.profile.description.to_lowercase().contains(&query.to_lowercase());
                let tag_match = tags.is_empty() || tags.iter().any(|tag| entry.metadata.tags.contains(tag));

                (name_match || desc_match) && tag_match
            })
            .cloned()
            .collect()
    }

    pub async fn get_profile(&self, name: &str) -> Option<OptimizationProfile> {
        let registry = self.registry.read().await;
        registry.profiles.get(name).map(|entry| entry.profile.clone())
    }

    pub async fn install_profile(&self, name: &str) -> Result<()> {
        let registry = self.registry.read().await;
        let entry = registry.profiles.get(name)
            .ok_or_else(|| anyhow::anyhow!("Profile not found: {}", name))?;

        validation::validate_profile(&entry.profile)?;
        validation::check_system_requirements(&entry.metadata.min_requirements)?;

        let profile_path = self.cache_dir.join(format!("{}.toml", name));
        let content = toml::to_string_pretty(&entry.profile)?;
        tokio::fs::write(profile_path, content).await?;

        Ok(())
    }

    pub async fn create_user_profile(&self, profile: OptimizationProfile) -> Result<()> {
        validation::validate_profile(&profile)?;

        let profile_path = self.user_profiles_dir.join(format!("{}.toml", profile.name));
        let content = toml::to_string_pretty(&profile)?;
        tokio::fs::write(profile_path, content).await?;

        let mut registry = self.registry.write().await;
        registry.user_profiles.insert(profile.name.clone(), profile);

        Ok(())
    }

    pub async fn share_profile(&self, profile_name: &str, repository: &str) -> Result<()> {
        let registry = self.registry.read().await;
        let profile = registry.user_profiles.get(profile_name)
            .ok_or_else(|| anyhow::anyhow!("User profile not found: {}", profile_name))?;

        let repo = registry.repositories.iter()
            .find(|r| r.name == repository)
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repository))?;

        community::submit_profile(profile, repo).await?;

        Ok(())
    }

    pub async fn rate_profile(&self, profile_name: &str, rating: f32) -> Result<()> {
        if !(1.0..=5.0).contains(&rating) {
            return Err(anyhow::anyhow!("Rating must be between 1.0 and 5.0"));
        }

        // Submit rating to community system
        community::submit_rating(profile_name, rating).await?;

        Ok(())
    }

    pub async fn report_profile(&self, profile_name: &str, reason: &str) -> Result<()> {
        community::report_profile(profile_name, reason).await?;
        Ok(())
    }

    pub async fn list_by_game(&self, game_title: &str) -> Vec<ProfileEntry> {
        let registry = self.registry.read().await;

        registry.profiles
            .values()
            .filter(|entry| {
                entry.metadata.compatible_games.iter()
                    .any(|game| game.to_lowercase().contains(&game_title.to_lowercase()))
            })
            .cloned()
            .collect()
    }

    pub async fn get_trending_profiles(&self, limit: usize) -> Vec<ProfileEntry> {
        let registry = self.registry.read().await;

        let mut profiles: Vec<_> = registry.profiles.values().collect();
        profiles.sort_by(|a, b| {
            let score_a = a.metadata.downloads as f32 + a.metadata.rating * 100.0;
            let score_b = b.metadata.downloads as f32 + b.metadata.rating * 100.0;
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        profiles.into_iter().take(limit).cloned().collect()
    }

    async fn load_user_profiles(&self) -> Result<()> {
        if !self.user_profiles_dir.exists() {
            tokio::fs::create_dir_all(&self.user_profiles_dir).await?;
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&self.user_profiles_dir).await?;
        let mut registry = self.registry.write().await;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(extension) = entry.path().extension() {
                if extension == "toml" {
                    let content = tokio::fs::read_to_string(entry.path()).await?;
                    if let Ok(profile) = toml::from_str::<OptimizationProfile>(&content) {
                        registry.user_profiles.insert(profile.name.clone(), profile);
                    }
                }
            }
        }

        Ok(())
    }

    async fn sync_repositories(&self) -> Result<()> {
        let registry_clone = Arc::clone(&self.registry);
        let repositories = {
            let registry = registry_clone.read().await;
            registry.repositories.clone()
        };

        for repo in repositories {
            if repo.enabled {
                if let Ok(profiles) = repository::fetch_profiles(&repo).await {
                    let mut registry = registry_clone.write().await;
                    for (name, entry) in profiles {
                        registry.profiles.insert(name, entry);
                    }
                }
            }
        }

        Ok(())
    }
}

impl ProfileRegistry {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            repositories: vec![
                ProfileRepository {
                    name: "official".to_string(),
                    url: "https://profiles.bolt.dev".to_string(),
                    trusted: true,
                    enabled: true,
                },
                ProfileRepository {
                    name: "community".to_string(),
                    url: "https://community.bolt.dev/profiles".to_string(),
                    trusted: false,
                    enabled: true,
                },
                ProfileRepository {
                    name: "gaming-hub".to_string(),
                    url: "https://gaming.bolt.dev/profiles".to_string(),
                    trusted: false,
                    enabled: true,
                },
            ],
            user_profiles: HashMap::new(),
        }
    }
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}