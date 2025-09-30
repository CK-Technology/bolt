use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub user_id: String,
    pub user_email: Option<String>,
    pub display_name: Option<String>,
    pub telemetry_enabled: bool,
    pub analytics_enabled: bool,
    pub auto_update: bool,
}

impl UserConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&config_path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;

        Ok(config_dir.join("bolt").join("user.toml"))
    }

    pub fn set_user_email(&mut self, email: String) -> Result<()> {
        self.user_email = Some(email);
        self.save()
    }

    pub fn set_display_name(&mut self, name: String) -> Result<()> {
        self.display_name = Some(name);
        self.save()
    }

    pub fn enable_telemetry(&mut self, enabled: bool) -> Result<()> {
        self.telemetry_enabled = enabled;
        self.save()
    }

    pub fn enable_analytics(&mut self, enabled: bool) -> Result<()> {
        self.analytics_enabled = enabled;
        self.save()
    }

    pub fn set_auto_update(&mut self, enabled: bool) -> Result<()> {
        self.auto_update = enabled;
        self.save()
    }

    pub fn get_user_id_or_anonymous(&self) -> String {
        if self.user_id.is_empty() {
            "anonymous".to_string()
        } else {
            self.user_id.clone()
        }
    }

    pub fn get_user_email_or_default(&self) -> String {
        self.user_email
            .clone()
            .unwrap_or_else(|| "user@example.com".to_string())
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            user_id: uuid::Uuid::new_v4().to_string(),
            user_email: None,
            display_name: None,
            telemetry_enabled: false,
            analytics_enabled: false,
            auto_update: true,
        }
    }
}
