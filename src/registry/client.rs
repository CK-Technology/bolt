use anyhow::Result;
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use super::*;

pub struct DriftClient {
    client: Client,
    base_url: String,
    auth: Option<RegistryAuth>,
}

impl DriftClient {
    pub fn new(registry: &DriftRegistry) -> Self {
        Self {
            client: Client::new(),
            base_url: registry.url.clone(),
            auth: registry.auth.clone(),
        }
    }

    pub async fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        let auth_request = serde_json::json!({
            "username": username,
            "password": password
        });

        let response = self
            .client
            .post(&format!("{}{}", self.base_url, endpoints::AUTH_LOGIN))
            .json(&auth_request)
            .send()
            .await?;

        if response.status().is_success() {
            let auth_response: serde_json::Value = response.json().await?;
            if let Some(token) = auth_response["token"].as_str() {
                self.auth = Some(RegistryAuth {
                    username: username.to_string(),
                    password: password.to_string(),
                    token: Some(token.to_string()),
                });
                info!("✅ Successfully authenticated with Drift registry");
            }
        } else {
            return Err(anyhow::anyhow!(
                "Authentication failed: {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Profile Management

    pub async fn list_profiles(
        &self,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<ProfileListResponse> {
        let mut url = format!("{}{}", self.base_url, endpoints::PROFILES_LIST);

        let mut params = Vec::new();
        if let Some(page) = page {
            params.push(format!("page={}", page));
        }
        if let Some(per_page) = per_page {
            params.push(format!("per_page={}", per_page));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.get(&url).await?;
        self.parse_response(response).await
    }

    pub async fn search_profiles(
        &self,
        request: &SearchRequest,
    ) -> Result<SearchResponse<ProfileSummary>> {
        let response = self
            .client
            .post(&format!("{}{}", self.base_url, endpoints::PROFILES_SEARCH))
            .json(request)
            .send()
            .await?;

        self.parse_response(response).await
    }

    pub async fn get_profile(&self, name: &str) -> Result<ProfileDetail> {
        let url = format!(
            "{}{}",
            self.base_url,
            endpoints::PROFILES_GET.replace("{name}", name)
        );
        let response = self.get(&url).await?;
        self.parse_response(response).await
    }

    pub async fn download_profile(&self, name: &str) -> Result<Vec<u8>> {
        let url = format!(
            "{}{}",
            self.base_url,
            endpoints::PROFILES_DOWNLOAD.replace("{name}", name)
        );
        let response = self.get(&url).await?;

        if response.status().is_success() {
            Ok(response.bytes().await?.to_vec())
        } else {
            Err(anyhow::anyhow!(
                "Failed to download profile: {}",
                response.status()
            ))
        }
    }

    pub async fn upload_profile(&self, request: &ProfileUploadRequest) -> Result<()> {
        let response = self
            .client
            .post(&format!("{}{}", self.base_url, endpoints::PROFILES_UPLOAD))
            .header("Authorization", self.get_auth_header()?)
            .header("Content-Type", media_types::BOLT_PROFILE)
            .json(request)
            .send()
            .await?;

        if response.status().is_success() {
            info!("✅ Profile uploaded successfully");
            Ok(())
        } else {
            let error: ApiError = self.parse_response(response).await?;
            Err(anyhow::anyhow!("Upload failed: {}", error))
        }
    }

    pub async fn delete_profile(&self, name: &str) -> Result<()> {
        let url = format!(
            "{}{}",
            self.base_url,
            endpoints::PROFILES_DELETE.replace("{name}", name)
        );
        let response = self
            .client
            .delete(&url)
            .header("Authorization", self.get_auth_header()?)
            .send()
            .await?;

        if response.status().is_success() {
            info!("✅ Profile deleted successfully");
            Ok(())
        } else {
            Err(anyhow::anyhow!("Delete failed: {}", response.status()))
        }
    }

    /// Plugin Management

    pub async fn list_plugins(
        &self,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<PluginListResponse> {
        let mut url = format!("{}{}", self.base_url, endpoints::PLUGINS_LIST);

        let mut params = Vec::new();
        if let Some(page) = page {
            params.push(format!("page={}", page));
        }
        if let Some(per_page) = per_page {
            params.push(format!("per_page={}", per_page));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.get(&url).await?;
        self.parse_response(response).await
    }

    pub async fn search_plugins(
        &self,
        request: &SearchRequest,
    ) -> Result<SearchResponse<PluginSummary>> {
        let response = self
            .client
            .post(&format!("{}{}", self.base_url, endpoints::PLUGINS_SEARCH))
            .json(request)
            .send()
            .await?;

        self.parse_response(response).await
    }

    pub async fn get_plugin(&self, name: &str) -> Result<PluginDetail> {
        let url = format!(
            "{}{}",
            self.base_url,
            endpoints::PLUGINS_GET.replace("{name}", name)
        );
        let response = self.get(&url).await?;
        self.parse_response(response).await
    }

    pub async fn download_plugin(&self, name: &str) -> Result<Vec<u8>> {
        let url = format!(
            "{}{}",
            self.base_url,
            endpoints::PLUGINS_DOWNLOAD.replace("{name}", name)
        );
        let response = self.get(&url).await?;

        if response.status().is_success() {
            Ok(response.bytes().await?.to_vec())
        } else {
            Err(anyhow::anyhow!(
                "Failed to download plugin: {}",
                response.status()
            ))
        }
    }

    pub async fn upload_plugin(&self, request: &PluginUploadRequest) -> Result<()> {
        let response = self
            .client
            .post(&format!("{}{}", self.base_url, endpoints::PLUGINS_UPLOAD))
            .header("Authorization", self.get_auth_header()?)
            .header("Content-Type", media_types::BOLT_PLUGIN_BINARY)
            .json(request)
            .send()
            .await?;

        if response.status().is_success() {
            info!("✅ Plugin uploaded successfully");
            Ok(())
        } else {
            let error: ApiError = self.parse_response(response).await?;
            Err(anyhow::anyhow!("Upload failed: {}", error))
        }
    }

    /// Metrics

    pub async fn get_metrics(&self) -> Result<RegistryMetrics> {
        let response = self
            .get(&format!("{}{}", self.base_url, endpoints::METRICS_OVERVIEW))
            .await?;
        self.parse_response(response).await
    }

    /// Health Check

    pub async fn health_check(&self) -> Result<serde_json::Value> {
        let response = self
            .get(&format!("{}{}", self.base_url, endpoints::HEALTH))
            .await?;
        self.parse_response(response).await
    }

    pub async fn get_version(&self) -> Result<serde_json::Value> {
        let response = self
            .get(&format!("{}{}", self.base_url, endpoints::VERSION))
            .await?;
        self.parse_response(response).await
    }

    /// Utility Methods

    async fn get(&self, url: &str) -> Result<Response> {
        let mut request = self.client.get(url);

        if let Some(auth) = &self.auth {
            if let Some(token) = &auth.token {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        Ok(request.send().await?)
    }

    async fn parse_response<T: DeserializeOwned>(&self, response: Response) -> Result<T> {
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let error_text = response.text().await?;

            // Try to parse as ApiError first
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                Err(anyhow::anyhow!("API Error: {}", api_error))
            } else {
                Err(anyhow::anyhow!("HTTP Error {}: {}", status, error_text))
            }
        }
    }

    fn get_auth_header(&self) -> Result<String> {
        if let Some(auth) = &self.auth {
            if let Some(token) = &auth.token {
                Ok(format!("Bearer {}", token))
            } else {
                Err(anyhow::anyhow!("No authentication token available"))
            }
        } else {
            Err(anyhow::anyhow!("Not authenticated"))
        }
    }
}

/// High-level convenience methods for common operations

impl DriftClient {
    /// Install a profile to local system
    pub async fn install_profile(&self, name: &str, install_path: &std::path::Path) -> Result<()> {
        info!("📦 Installing profile: {}", name);

        // Get profile details first
        let profile_detail = self.get_profile(name).await?;

        // Validate system requirements
        self.validate_system_requirements(&profile_detail.metadata.system_requirements)?;

        // Download profile data
        let profile_data = self.download_profile(name).await?;

        // Save to local filesystem
        let profile_path = install_path.join(format!("{}.toml", name));
        tokio::fs::write(profile_path, profile_data).await?;

        info!("✅ Profile '{}' installed successfully", name);
        Ok(())
    }

    /// Install a plugin to local system
    pub async fn install_plugin(&self, name: &str, install_path: &std::path::Path) -> Result<()> {
        info!("🔌 Installing plugin: {}", name);

        // Get plugin details
        let plugin_detail = self.get_plugin(name).await?;

        // Download plugin binary
        let plugin_data = self.download_plugin(name).await?;

        // Create plugin directory
        let plugin_dir = install_path.join(&name);
        tokio::fs::create_dir_all(&plugin_dir).await?;

        // Save plugin manifest
        let manifest_path = plugin_dir.join("plugin.toml");
        let manifest_content = toml::to_string_pretty(&plugin_detail.manifest)?;
        tokio::fs::write(manifest_path, manifest_content).await?;

        // Save plugin binary
        let binary_path = plugin_dir.join(&plugin_detail.manifest.entry_point);
        tokio::fs::write(binary_path, plugin_data).await?;

        info!("✅ Plugin '{}' installed successfully", name);
        Ok(())
    }

    fn validate_system_requirements(&self, requirements: &SystemRequirements) -> Result<()> {
        // Basic system validation - would be expanded with actual system detection
        if let Some(min_cores) = requirements.min_cpu_cores {
            let available_cores = num_cpus::get() as u32;
            if available_cores < min_cores {
                return Err(anyhow::anyhow!(
                    "Insufficient CPU cores: required {}, available {}",
                    min_cores,
                    available_cores
                ));
            }
        }

        // Additional validation would go here
        Ok(())
    }
}
