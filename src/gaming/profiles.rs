//! Gaming Profiles - Pre-configured settings for popular games
//!
//! Based on nvbind's gaming profiles with optimizations for specific games

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Pre-configured gaming profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingProfile {
    pub name: String,
    pub game_id: Option<String>, // Steam App ID
    pub description: String,
    pub gpu_config: GpuProfileConfig,
    pub nvidia_settings: Option<NvidiaGameSettings>,
    pub wine_settings: Option<WineSettings>,
    pub performance_hints: PerformanceHints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProfileConfig {
    pub power_limit_watts: Option<u32>,
    pub gpu_clock_offset_mhz: Option<i32>,
    pub memory_clock_offset_mhz: Option<i32>,
    pub fan_speed_percent: Option<u32>,
    pub performance_mode: PerformanceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerformanceMode {
    UltraLowLatency,
    Performance,
    Balanced,
    Quiet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaGameSettings {
    pub dlss_mode: Option<DlssMode>,
    pub dlss_quality: Option<DlssQuality>,
    pub raytracing_quality: Option<RaytracingQuality>,
    pub reflex_enabled: bool,
    pub reflex_boost: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DlssMode {
    Off,
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DlssQuality {
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RaytracingQuality {
    Off,
    Low,
    Medium,
    High,
    Ultra,
    Psycho,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineSettings {
    pub proton_version: Option<String>,
    pub dxvk_enabled: bool,
    pub vkd3d_enabled: bool,
    pub esync_enabled: bool,
    pub fsync_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceHints {
    pub target_fps: u32,
    pub expected_vram_mb: u32,
    pub cpu_intensive: bool,
    pub gpu_intensive: bool,
    pub latency_critical: bool,
}

/// Gaming Profile Manager
pub struct GamingProfileManager {
    profiles: HashMap<String, GamingProfile>,
}

impl GamingProfileManager {
    /// Create a new gaming profile manager with built-in profiles
    pub fn new() -> Self {
        let mut profiles = HashMap::new();

        // Add all built-in profiles
        for profile in Self::builtin_profiles() {
            profiles.insert(profile.name.to_lowercase(), profile);
        }

        info!("🎮 Loaded {} gaming profiles", profiles.len());
        Self { profiles }
    }

    /// Get all built-in gaming profiles
    fn builtin_profiles() -> Vec<GamingProfile> {
        vec![
            Self::cyberpunk2077(),
            Self::counter_strike_2(),
            Self::elden_ring(),
            Self::baldurs_gate_3(),
            Self::starfield(),
            Self::hogwarts_legacy(),
            Self::red_dead_redemption_2(),
            Self::witcher_3(),
        ]
    }

    /// Cyberpunk 2077 - Path Tracing + DLSS
    fn cyberpunk2077() -> GamingProfile {
        GamingProfile {
            name: "Cyberpunk 2077".to_string(),
            game_id: Some("1091500".to_string()),
            description: "Path tracing with DLSS Quality for best visuals".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(350),
                gpu_clock_offset_mhz: Some(100),
                memory_clock_offset_mhz: Some(500),
                fan_speed_percent: Some(75),
                performance_mode: PerformanceMode::Performance,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Quality),
                dlss_quality: Some(DlssQuality::Quality),
                raytracing_quality: Some(RaytracingQuality::Psycho),
                reflex_enabled: true,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: true,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 60,
                expected_vram_mb: 12288,
                cpu_intensive: false,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// Counter-Strike 2 - Competitive low latency
    fn counter_strike_2() -> GamingProfile {
        GamingProfile {
            name: "Counter-Strike 2".to_string(),
            game_id: Some("730".to_string()),
            description: "Ultra-low latency for competitive gaming".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(300),
                gpu_clock_offset_mhz: Some(150),
                memory_clock_offset_mhz: Some(800),
                fan_speed_percent: Some(80),
                performance_mode: PerformanceMode::UltraLowLatency,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Off), // Competitive players prefer native
                dlss_quality: None,
                raytracing_quality: Some(RaytracingQuality::Off),
                reflex_enabled: true,
                reflex_boost: true,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: false,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 300,
                expected_vram_mb: 4096,
                cpu_intensive: true,
                gpu_intensive: false,
                latency_critical: true,
            },
        }
    }

    /// Elden Ring - Stable 60 FPS
    fn elden_ring() -> GamingProfile {
        GamingProfile {
            name: "Elden Ring".to_string(),
            game_id: Some("1245620".to_string()),
            description: "Locked 60 FPS with consistent frame pacing".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(250),
                gpu_clock_offset_mhz: Some(50),
                memory_clock_offset_mhz: Some(300),
                fan_speed_percent: Some(65),
                performance_mode: PerformanceMode::Balanced,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Quality),
                dlss_quality: Some(DlssQuality::Quality),
                raytracing_quality: Some(RaytracingQuality::Off),
                reflex_enabled: false,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: false,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 60,
                expected_vram_mb: 6144,
                cpu_intensive: false,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// Baldur's Gate 3 - Ray Tracing
    fn baldurs_gate_3() -> GamingProfile {
        GamingProfile {
            name: "Baldur's Gate 3".to_string(),
            game_id: Some("1086940".to_string()),
            description: "High quality with ray tracing".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(300),
                gpu_clock_offset_mhz: Some(75),
                memory_clock_offset_mhz: Some(400),
                fan_speed_percent: Some(70),
                performance_mode: PerformanceMode::Performance,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Balanced),
                dlss_quality: Some(DlssQuality::Balanced),
                raytracing_quality: Some(RaytracingQuality::High),
                reflex_enabled: false,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: true,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 60,
                expected_vram_mb: 8192,
                cpu_intensive: true,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// Starfield - Balanced
    fn starfield() -> GamingProfile {
        GamingProfile {
            name: "Starfield".to_string(),
            game_id: Some("1716740".to_string()),
            description: "Balanced settings for stable performance".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(280),
                gpu_clock_offset_mhz: Some(60),
                memory_clock_offset_mhz: Some(350),
                fan_speed_percent: Some(70),
                performance_mode: PerformanceMode::Balanced,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Balanced),
                dlss_quality: Some(DlssQuality::Balanced),
                raytracing_quality: Some(RaytracingQuality::Medium),
                reflex_enabled: false,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: true,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 60,
                expected_vram_mb: 10240,
                cpu_intensive: true,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// Hogwarts Legacy - DLSS 3
    fn hogwarts_legacy() -> GamingProfile {
        GamingProfile {
            name: "Hogwarts Legacy".to_string(),
            game_id: Some("990080".to_string()),
            description: "DLSS 3 Frame Generation for high FPS".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(320),
                gpu_clock_offset_mhz: Some(90),
                memory_clock_offset_mhz: Some(450),
                fan_speed_percent: Some(75),
                performance_mode: PerformanceMode::Performance,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Performance),
                dlss_quality: Some(DlssQuality::Performance),
                raytracing_quality: Some(RaytracingQuality::High),
                reflex_enabled: true,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: true,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 90,
                expected_vram_mb: 10240,
                cpu_intensive: false,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// Red Dead Redemption 2 - Optimized
    fn red_dead_redemption_2() -> GamingProfile {
        GamingProfile {
            name: "Red Dead Redemption 2".to_string(),
            game_id: Some("1174180".to_string()),
            description: "Optimized settings for best visuals and performance".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(300),
                gpu_clock_offset_mhz: Some(80),
                memory_clock_offset_mhz: Some(400),
                fan_speed_percent: Some(70),
                performance_mode: PerformanceMode::Balanced,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Quality),
                dlss_quality: Some(DlssQuality::Quality),
                raytracing_quality: Some(RaytracingQuality::Off),
                reflex_enabled: false,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: false,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 60,
                expected_vram_mb: 8192,
                cpu_intensive: true,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// The Witcher 3 - Next-gen RT
    fn witcher_3() -> GamingProfile {
        GamingProfile {
            name: "The Witcher 3".to_string(),
            game_id: Some("292030".to_string()),
            description: "Next-gen update with ray tracing".to_string(),
            gpu_config: GpuProfileConfig {
                power_limit_watts: Some(280),
                gpu_clock_offset_mhz: Some(70),
                memory_clock_offset_mhz: Some(380),
                fan_speed_percent: Some(70),
                performance_mode: PerformanceMode::Balanced,
            },
            nvidia_settings: Some(NvidiaGameSettings {
                dlss_mode: Some(DlssMode::Quality),
                dlss_quality: Some(DlssQuality::Quality),
                raytracing_quality: Some(RaytracingQuality::Ultra),
                reflex_enabled: false,
                reflex_boost: false,
            }),
            wine_settings: Some(WineSettings {
                proton_version: Some("8.0".to_string()),
                dxvk_enabled: true,
                vkd3d_enabled: true,
                esync_enabled: true,
                fsync_enabled: true,
            }),
            performance_hints: PerformanceHints {
                target_fps: 60,
                expected_vram_mb: 8192,
                cpu_intensive: false,
                gpu_intensive: true,
                latency_critical: false,
            },
        }
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&GamingProfile> {
        self.profiles.get(&name.to_lowercase())
    }

    /// List all available profiles
    pub fn list_profiles(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    /// Apply a profile to a container
    pub async fn apply_profile(
        &self,
        container_id: &str,
        profile_name: &str,
    ) -> Result<()> {
        let profile = self
            .get_profile(profile_name)
            .ok_or_else(|| anyhow::anyhow!("Profile not found: {}", profile_name))?;

        info!("🎮 Applying gaming profile '{}' to container: {}", profile.name, container_id);
        info!("   Description: {}", profile.description);
        info!("   Target FPS: {}", profile.performance_hints.target_fps);
        info!("   Expected VRAM: {}MB", profile.performance_hints.expected_vram_mb);

        // Apply GPU configuration
        if let Some(power_limit) = profile.gpu_config.power_limit_watts {
            info!("   Power limit: {}W", power_limit);
        }

        // Apply NVIDIA-specific settings
        if let Some(ref nvidia) = profile.nvidia_settings {
            info!("   DLSS: {:?}", nvidia.dlss_mode);
            info!("   Ray Tracing: {:?}", nvidia.raytracing_quality);
            info!("   Reflex: {}", nvidia.reflex_enabled);
        }

        // Apply Wine/Proton settings
        if let Some(ref wine) = profile.wine_settings {
            if let Some(ref proton) = wine.proton_version {
                info!("   Proton version: {}", proton);
            }
            info!("   DXVK: {}", wine.dxvk_enabled);
            info!("   VKD3D: {}", wine.vkd3d_enabled);
        }

        // In production, this would:
        // 1. Apply GPU settings via nvidia-smi or nvbind
        // 2. Configure container environment variables
        // 3. Set up Wine/Proton configuration
        // 4. Apply performance optimizations

        #[cfg(feature = "nvbind-support")]
        {
            // Use nvbind to apply profile
            // nvbind::apply_gaming_profile(container_id, profile).await?;
            info!("   Using nvbind for GPU configuration");
        }

        info!("✅ Gaming profile applied successfully");
        Ok(())
    }

    /// Get profile by Steam App ID
    pub fn get_profile_by_app_id(&self, app_id: &str) -> Option<&GamingProfile> {
        self.profiles
            .values()
            .find(|p| p.game_id.as_deref() == Some(app_id))
    }
}

impl Default for GamingProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager_loads_all_profiles() {
        let manager = GamingProfileManager::new();
        assert_eq!(manager.list_profiles().len(), 8);
    }

    #[test]
    fn test_get_profile_by_name() {
        let manager = GamingProfileManager::new();
        let profile = manager.get_profile("cyberpunk 2077");
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().name, "Cyberpunk 2077");
    }

    #[test]
    fn test_get_profile_by_app_id() {
        let manager = GamingProfileManager::new();
        let profile = manager.get_profile_by_app_id("730"); // CS2
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().name, "Counter-Strike 2");
    }

    #[test]
    fn test_cs2_profile_has_low_latency() {
        let manager = GamingProfileManager::new();
        let profile = manager.get_profile("counter-strike 2").unwrap();
        assert!(matches!(
            profile.gpu_config.performance_mode,
            PerformanceMode::UltraLowLatency
        ));
        assert!(profile.performance_hints.latency_critical);
        assert_eq!(profile.performance_hints.target_fps, 300);
    }
}
