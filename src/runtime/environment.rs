use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::debug;

/// Safe environment variable manager that avoids unsafe operations
#[derive(Debug, Clone)]
pub struct EnvironmentManager {
    /// Container-scoped environment variables
    container_env: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
    /// Process-local environment for container configurations
    process_env: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvironmentManager {
    pub fn new() -> Self {
        Self {
            container_env: Arc::new(RwLock::new(HashMap::new())),
            process_env: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set environment variable for a specific container (safe alternative to std::env::set_var)
    pub fn set_container_env(
        &self,
        container_id: &str,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<()> {
        let key = key.into();
        let value = value.into();

        let mut env = self
            .container_env
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire environment lock"))?;
        env.entry(container_id.to_string())
            .or_default()
            .insert(key.clone(), value.clone());

        debug!("Set container env {}[{}] = {}", container_id, key, value);
        Ok(())
    }

    /// Set multiple environment variables for a container
    pub fn set_container_env_batch(
        &self,
        container_id: &str,
        vars: HashMap<String, String>,
    ) -> Result<()> {
        let mut env = self
            .container_env
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire environment lock"))?;
        let container_env = env.entry(container_id.to_string()).or_default();

        for (key, value) in vars {
            debug!("Set container env {}[{}] = {}", container_id, key, value);
            container_env.insert(key, value);
        }
        Ok(())
    }

    /// Get environment variables for a container
    pub fn get_container_env(&self, container_id: &str) -> Result<HashMap<String, String>> {
        let env = self
            .container_env
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire environment lock"))?;
        Ok(env.get(container_id).cloned().unwrap_or_default())
    }

    /// Remove all environment variables for a container
    pub fn clear_container_env(&self, container_id: &str) -> Result<()> {
        let mut env = self
            .container_env
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire environment lock"))?;
        env.remove(container_id);
        debug!("Cleared environment for container {}", container_id);
        Ok(())
    }

    /// Generate container environment as Vec<String> for process execution
    pub fn to_env_vec(&self, container_id: &str) -> Result<Vec<String>> {
        let container_env = self.get_container_env(container_id)?;
        let process_env = self
            .process_env
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire environment lock"))?;

        let mut env_vec = Vec::new();

        // Add process-level environment first
        for (key, value) in process_env.iter() {
            env_vec.push(format!("{}={}", key, value));
        }

        // Add container-specific environment (can override process env)
        for (key, value) in container_env.iter() {
            env_vec.push(format!("{}={}", key, value));
        }

        Ok(env_vec)
    }

    /// Set gaming optimizations for Wayland/KDE (safe replacement for unsafe env operations)
    pub fn configure_gaming_environment(
        &self,
        container_id: &str,
        desktop_env: &str,
        wayland_display: &str,
    ) -> Result<()> {
        let mut gaming_env = HashMap::new();

        // Core Wayland environment
        gaming_env.insert("WAYLAND_DISPLAY".to_string(), wayland_display.to_string());
        gaming_env.insert("GDK_BACKEND".to_string(), "wayland".to_string());
        gaming_env.insert("QT_QPA_PLATFORM".to_string(), "wayland".to_string());
        gaming_env.insert("CLUTTER_BACKEND".to_string(), "wayland".to_string());
        gaming_env.insert("SDL_VIDEODRIVER".to_string(), "wayland".to_string());

        // Gaming optimizations
        gaming_env.insert("WAYLAND_GAMING_OPTIMIZATIONS".to_string(), "1".to_string());
        gaming_env.insert("WAYLAND_DISABLE_VSYNC".to_string(), "1".to_string());
        gaming_env.insert("WAYLAND_LOW_LATENCY".to_string(), "1".to_string());

        // Hardware acceleration
        gaming_env.insert("LIBGL_ALWAYS_SOFTWARE".to_string(), "0".to_string());
        gaming_env.insert("EGL_PLATFORM".to_string(), "wayland".to_string());

        // Desktop environment specific optimizations
        match desktop_env.to_lowercase().as_str() {
            "kde" | "plasma" => {
                self.configure_kde_gaming_environment(&mut gaming_env)?;
            }
            "gnome" => {
                self.configure_gnome_gaming_environment(&mut gaming_env)?;
            }
            _ => {
                self.configure_generic_wayland_gaming(&mut gaming_env)?;
            }
        }

        self.set_container_env_batch(container_id, gaming_env)
    }

    fn configure_kde_gaming_environment(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // KDE gaming mode
        env.insert("KDE_GAMING_MODE".to_string(), "1".to_string());
        env.insert("PLASMA_GAMING_MODE".to_string(), "1".to_string());

        // KWin optimizations
        env.insert("KWIN_TRIPLE_BUFFER".to_string(), "1".to_string());
        env.insert("KWIN_LOWLATENCY".to_string(), "1".to_string());
        env.insert("KWIN_EXPLICIT_SYNC".to_string(), "1".to_string());
        env.insert("KWIN_ALLOW_TEARING".to_string(), "1".to_string());
        env.insert("KWIN_DRM_USE_MODIFIERS".to_string(), "1".to_string());

        // VRR support
        env.insert("KWIN_VRR".to_string(), "1".to_string());
        env.insert("KWIN_ADAPTIVE_SYNC".to_string(), "1".to_string());

        // Qt gaming optimizations
        env.insert(
            "QT_WAYLAND_DISABLE_WINDOWDECORATION".to_string(),
            "1".to_string(),
        );
        env.insert("QT_WAYLAND_FORCE_DPI".to_string(), "96".to_string());

        Ok(())
    }

    fn configure_gnome_gaming_environment(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // Mutter optimizations
        env.insert(
            "MUTTER_DEBUG_FORCE_KMS_MODE".to_string(),
            "simple".to_string(),
        );
        env.insert(
            "MUTTER_DEBUG_ENABLE_ATOMIC_KMS".to_string(),
            "1".to_string(),
        );
        env.insert("MUTTER_DISABLE_VSYNC".to_string(), "1".to_string());
        env.insert("GNOME_GAMING_OPTIMIZATIONS".to_string(), "1".to_string());

        // GNOME specific gaming features
        env.insert("GNOME_SHELL_DISABLE_EFFECTS".to_string(), "1".to_string());

        Ok(())
    }

    fn configure_generic_wayland_gaming(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // wlroots-based compositors (Sway, Hyprland, etc.)
        env.insert("WLR_RENDERER".to_string(), "vulkan".to_string());
        env.insert("WLR_NO_HARDWARE_CURSORS".to_string(), "1".to_string());
        env.insert("WLR_DRM_NO_ATOMIC".to_string(), "0".to_string());
        env.insert("WLR_GAMING_OPTIMIZATIONS".to_string(), "1".to_string());

        // Generic optimizations
        env.insert("WAYLAND_DEBUG".to_string(), "0".to_string());
        env.insert("WAYLAND_GAMING_MODE".to_string(), "1".to_string());

        Ok(())
    }

    /// Configure AI/ML environment optimizations
    pub fn configure_ai_environment(&self, container_id: &str, ai_backend: &str) -> Result<()> {
        let mut ai_env = HashMap::new();

        // Common AI optimizations
        ai_env.insert(
            "PYTORCH_CUDA_ALLOC_CONF".to_string(),
            "max_split_size_mb:128".to_string(),
        );
        ai_env.insert("CUDA_LAUNCH_BLOCKING".to_string(), "0".to_string());
        ai_env.insert("TOKENIZERS_PARALLELISM".to_string(), "true".to_string());

        match ai_backend.to_lowercase().as_str() {
            "ollama" => {
                self.configure_ollama_environment(&mut ai_env)?;
            }
            "localai" => {
                self.configure_localai_environment(&mut ai_env)?;
            }
            "tensorflow" => {
                self.configure_tensorflow_environment(&mut ai_env)?;
            }
            "pytorch" => {
                self.configure_pytorch_environment(&mut ai_env)?;
            }
            _ => {
                // Generic AI optimizations
                ai_env.insert("OMP_NUM_THREADS".to_string(), num_cpus::get().to_string());
            }
        }

        self.set_container_env_batch(container_id, ai_env)
    }

    fn configure_ollama_environment(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // Ollama-specific optimizations
        env.insert("OLLAMA_HOST".to_string(), "0.0.0.0:11434".to_string());
        env.insert("OLLAMA_ORIGINS".to_string(), "*".to_string());
        env.insert("OLLAMA_MODELS".to_string(), "/app/models".to_string());
        env.insert("OLLAMA_NUM_PARALLEL".to_string(), "4".to_string());
        env.insert("OLLAMA_MAX_LOADED_MODELS".to_string(), "3".to_string());
        env.insert("OLLAMA_FLASH_ATTENTION".to_string(), "1".to_string());

        Ok(())
    }

    fn configure_localai_environment(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // LocalAI optimizations
        env.insert("LOCALAI_HOST".to_string(), "0.0.0.0:8080".to_string());
        env.insert("BUILD_TYPE".to_string(), "cublas".to_string());
        env.insert("GO_TAGS".to_string(), "tts".to_string());
        env.insert("REBUILD".to_string(), "true".to_string());

        Ok(())
    }

    fn configure_tensorflow_environment(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // TensorFlow GPU optimizations
        env.insert(
            "TF_GPU_ALLOCATOR".to_string(),
            "cuda_malloc_async".to_string(),
        );
        env.insert("TF_FORCE_GPU_ALLOW_GROWTH".to_string(), "true".to_string());
        env.insert(
            "TF_XLA_FLAGS".to_string(),
            "--tf_xla_enable_xla_devices".to_string(),
        );

        Ok(())
    }

    fn configure_pytorch_environment(&self, env: &mut HashMap<String, String>) -> Result<()> {
        // PyTorch optimizations
        env.insert(
            "TORCH_CUDA_ARCH_LIST".to_string(),
            "8.0;8.6;8.9;9.0".to_string(),
        );
        env.insert(
            "PYTORCH_CUDA_ALLOC_CONF".to_string(),
            "expandable_segments:True".to_string(),
        );
        env.insert("TORCH_CUDNN_V8_API_ENABLED".to_string(), "1".to_string());

        Ok(())
    }
}

/// Global environment manager instance
static ENVIRONMENT_MANAGER: std::sync::OnceLock<EnvironmentManager> = std::sync::OnceLock::new();

/// Get global environment manager instance
pub fn env_manager() -> &'static EnvironmentManager {
    ENVIRONMENT_MANAGER.get_or_init(EnvironmentManager::new)
}
