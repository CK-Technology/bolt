use crate::{BoltError, Result};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::fs;
use tokio::process::Command as AsyncCommand;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Audio subsystem to use
    pub subsystem: AudioSubsystem,
    /// Enable low-latency audio optimizations
    pub low_latency_mode: bool,
    /// Audio buffer size (lower = less latency, higher = less CPU usage)
    pub buffer_size: u32,
    /// Sample rate (44100, 48000, 96000, 192000)
    pub sample_rate: u32,
    /// Audio bit depth (16, 24, 32)
    pub bit_depth: u32,
    /// Enable audio passthrough for surround sound
    pub passthrough_enabled: bool,
    /// Enable gaming-specific audio optimizations
    pub gaming_optimizations: bool,
    /// Custom audio device to use
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioSubsystem {
    /// PipeWire (modern, low-latency)
    PipeWire,
    /// PulseAudio (traditional)
    PulseAudio,
    /// ALSA direct access
    ALSA,
    /// JACK (professional audio)
    JACK,
    /// Auto-detect best available
    Auto,
}

#[derive(Debug)]
pub struct AudioManager {
    config: AudioConfig,
    detected_subsystem: Arc<RwLock<AudioSubsystem>>,
    available_devices: Arc<RwLock<Vec<AudioDevice>>>,
    runtime_paths: Arc<RwLock<AudioRuntimePaths>>,
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub device_type: AudioDeviceType,
    pub channels: u32,
    pub sample_rate: u32,
    pub latency_ms: f64,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub enum AudioDeviceType {
    Output,
    Input,
    Duplex,
}

#[derive(Debug, Clone)]
pub struct AudioRuntimePaths {
    pub socket_path: PathBuf,
    pub config_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub pulse_server: Option<String>,
    pub pipewire_runtime: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            subsystem: AudioSubsystem::Auto,
            low_latency_mode: true,
            buffer_size: 128, // Low latency for gaming
            sample_rate: 48000, // Gaming standard
            bit_depth: 24, // High quality
            passthrough_enabled: true,
            gaming_optimizations: true,
            device_name: None,
        }
    }
}

impl AudioManager {
    pub async fn new(config: AudioConfig) -> Result<Self> {
        info!("🎵 Initializing Audio Manager");
        info!("   Subsystem: {:?}", config.subsystem);
        info!("   Low Latency: {} (buffer: {})", config.low_latency_mode, config.buffer_size);
        info!("   Sample Rate: {}Hz, Bit Depth: {}bit", config.sample_rate, config.bit_depth);

        // Detect available audio subsystem
        let detected_subsystem = Self::detect_audio_subsystem(&config).await?;
        info!("   Detected: {:?}", detected_subsystem);

        // Discover available audio devices
        let available_devices = Self::discover_audio_devices(&detected_subsystem).await?;
        info!("   Devices found: {}", available_devices.len());

        // Setup runtime paths
        let runtime_paths = Self::setup_runtime_paths(&detected_subsystem).await?;

        Ok(Self {
            config,
            detected_subsystem: Arc::new(RwLock::new(detected_subsystem)),
            available_devices: Arc::new(RwLock::new(available_devices)),
            runtime_paths: Arc::new(RwLock::new(runtime_paths)),
        })
    }

    async fn detect_audio_subsystem(config: &AudioConfig) -> Result<AudioSubsystem> {
        match config.subsystem {
            AudioSubsystem::Auto => {
                // Check for PipeWire first (most modern)
                if Self::is_pipewire_available().await {
                    info!("✅ PipeWire detected and available");
                    Ok(AudioSubsystem::PipeWire)
                }
                // Fallback to PulseAudio
                else if Self::is_pulseaudio_available().await {
                    info!("✅ PulseAudio detected and available");
                    Ok(AudioSubsystem::PulseAudio)
                }
                // Fallback to ALSA
                else if Self::is_alsa_available().await {
                    info!("✅ ALSA detected and available");
                    Ok(AudioSubsystem::ALSA)
                }
                else {
                    warn!("No suitable audio subsystem found, using ALSA");
                    Ok(AudioSubsystem::ALSA)
                }
            }
            specific => Ok(specific),
        }
    }

    async fn is_pipewire_available() -> bool {
        // Check if PipeWire daemon is running
        if let Ok(output) = AsyncCommand::new("systemctl")
            .args(&["--user", "is-active", "pipewire"])
            .output()
            .await
        {
            if output.status.success() {
                return true;
            }
        }

        // Check for PipeWire socket
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".to_string());
        let pipewire_socket = format!("{}/pipewire-0", runtime_dir);
        tokio::fs::metadata(&pipewire_socket).await.is_ok()
    }

    async fn is_pulseaudio_available() -> bool {
        // Check if PulseAudio server is running
        if let Ok(output) = AsyncCommand::new("pulseaudio")
            .arg("--check")
            .output()
            .await
        {
            return output.status.success();
        }

        // Check for PulseAudio socket
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".to_string());
        let pulse_socket = format!("{}/pulse/native", runtime_dir);
        tokio::fs::metadata(&pulse_socket).await.is_ok()
    }

    async fn is_alsa_available() -> bool {
        // ALSA is kernel-level, check for devices
        tokio::fs::metadata("/dev/snd").await.is_ok()
    }

    async fn discover_audio_devices(subsystem: &AudioSubsystem) -> Result<Vec<AudioDevice>> {
        match subsystem {
            AudioSubsystem::PipeWire => Self::discover_pipewire_devices().await,
            AudioSubsystem::PulseAudio => Self::discover_pulseaudio_devices().await,
            AudioSubsystem::ALSA => Self::discover_alsa_devices().await,
            _ => Ok(Vec::new()),
        }
    }

    async fn discover_pipewire_devices() -> Result<Vec<AudioDevice>> {
        let output = AsyncCommand::new("pw-cli")
            .args(&["list-objects"])
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let devices = Self::parse_pipewire_devices(&stdout);
                info!("   📻 PipeWire devices: {}", devices.len());
                Ok(devices)
            }
            _ => {
                warn!("Failed to enumerate PipeWire devices");
                Ok(Vec::new())
            }
        }
    }

    fn parse_pipewire_devices(output: &str) -> Vec<AudioDevice> {
        let mut devices = Vec::new();

        // Simplified parsing - in production would use proper PipeWire API
        for line in output.lines() {
            if line.contains("type:PipeWire:Interface:Node") {
                devices.push(AudioDevice {
                    name: "PipeWire Device".to_string(),
                    description: "Auto-detected PipeWire Audio Device".to_string(),
                    device_type: AudioDeviceType::Output,
                    channels: 2,
                    sample_rate: 48000,
                    latency_ms: 5.0, // PipeWire typical low latency
                    is_default: false,
                });
            }
        }

        devices
    }

    async fn discover_pulseaudio_devices() -> Result<Vec<AudioDevice>> {
        let output = AsyncCommand::new("pactl")
            .args(&["list", "sinks", "short"])
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let devices = Self::parse_pulseaudio_devices(&stdout);
                info!("   🔊 PulseAudio devices: {}", devices.len());
                Ok(devices)
            }
            _ => {
                warn!("Failed to enumerate PulseAudio devices");
                Ok(Vec::new())
            }
        }
    }

    fn parse_pulseaudio_devices(output: &str) -> Vec<AudioDevice> {
        let mut devices = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                devices.push(AudioDevice {
                    name: parts[1].to_string(),
                    description: parts.get(4).unwrap_or(&"PulseAudio Device").to_string(),
                    device_type: AudioDeviceType::Output,
                    channels: 2,
                    sample_rate: 44100,
                    latency_ms: 20.0, // PulseAudio typical latency
                    is_default: false,
                });
            }
        }

        devices
    }

    async fn discover_alsa_devices() -> Result<Vec<AudioDevice>> {
        let output = AsyncCommand::new("aplay")
            .args(&["-l"])
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let devices = Self::parse_alsa_devices(&stdout);
                info!("   🎶 ALSA devices: {}", devices.len());
                Ok(devices)
            }
            _ => {
                warn!("Failed to enumerate ALSA devices");
                Ok(Vec::new())
            }
        }
    }

    fn parse_alsa_devices(output: &str) -> Vec<AudioDevice> {
        let mut devices = Vec::new();

        for line in output.lines() {
            if line.starts_with("card") {
                devices.push(AudioDevice {
                    name: line.to_string(),
                    description: "ALSA Audio Device".to_string(),
                    device_type: AudioDeviceType::Output,
                    channels: 2,
                    sample_rate: 44100,
                    latency_ms: 10.0, // ALSA can be quite low latency
                    is_default: false,
                });
            }
        }

        devices
    }

    async fn setup_runtime_paths(subsystem: &AudioSubsystem) -> Result<AudioRuntimePaths> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| "/tmp".to_string());
        let runtime_path = PathBuf::from(&runtime_dir);

        let paths = match subsystem {
            AudioSubsystem::PipeWire => AudioRuntimePaths {
                socket_path: runtime_path.join("pipewire-0"),
                config_path: runtime_path.join("pipewire"),
                runtime_dir: runtime_path.clone(),
                pulse_server: None,
                pipewire_runtime: Some(runtime_dir.clone()),
            },
            AudioSubsystem::PulseAudio => AudioRuntimePaths {
                socket_path: runtime_path.join("pulse/native"),
                config_path: runtime_path.join("pulse"),
                runtime_dir: runtime_path.clone(),
                pulse_server: Some(format!("unix:{}/pulse/native", runtime_dir)),
                pipewire_runtime: None,
            },
            _ => AudioRuntimePaths {
                socket_path: PathBuf::from("/dev/snd"),
                config_path: runtime_path.join("alsa"),
                runtime_dir: runtime_path.clone(),
                pulse_server: None,
                pipewire_runtime: None,
            },
        };

        Ok(paths)
    }

    pub async fn configure_container_audio(&self, container_id: &str) -> Result<AudioContainerConfig> {
        info!("🎵 Configuring audio for container: {}", container_id);

        let subsystem = self.detected_subsystem.read().await;
        let paths = self.runtime_paths.read().await;

        let mut env_vars = HashMap::new();
        let mut volumes = Vec::new();
        let mut devices = Vec::new();

        // Configure based on detected subsystem
        match &*subsystem {
            AudioSubsystem::PipeWire => {
                self.configure_pipewire_container(&mut env_vars, &mut volumes, &paths).await;
            }
            AudioSubsystem::PulseAudio => {
                self.configure_pulseaudio_container(&mut env_vars, &mut volumes, &paths).await;
            }
            AudioSubsystem::ALSA => {
                self.configure_alsa_container(&mut env_vars, &mut volumes, &mut devices).await;
            }
            _ => {}
        }

        // Apply gaming optimizations
        if self.config.gaming_optimizations {
            self.apply_gaming_audio_optimizations(&mut env_vars).await;
        }

        // Apply low-latency optimizations
        if self.config.low_latency_mode {
            self.apply_low_latency_optimizations(&mut env_vars).await;
        }

        info!("✅ Audio configuration complete: {} env vars, {} volumes, {} devices",
              env_vars.len(), volumes.len(), devices.len());

        Ok(AudioContainerConfig {
            env_vars,
            volumes,
            devices,
            subsystem: subsystem.clone(),
        })
    }

    async fn configure_pipewire_container(
        &self,
        env_vars: &mut HashMap<String, String>,
        volumes: &mut Vec<String>,
        paths: &AudioRuntimePaths,
    ) {
        info!("📻 Configuring PipeWire container audio");

        // PipeWire environment variables
        env_vars.insert("PIPEWIRE_RUNTIME_DIR".to_string(),
                       paths.runtime_dir.to_string_lossy().to_string());

        if let Some(pipewire_runtime) = &paths.pipewire_runtime {
            env_vars.insert("XDG_RUNTIME_DIR".to_string(), pipewire_runtime.clone());
        }

        // PipeWire socket and config mounts
        volumes.push(format!("{}:/tmp/pipewire-runtime:rw",
                           paths.runtime_dir.to_string_lossy()));

        // PipeWire-specific optimizations
        env_vars.insert("PIPEWIRE_LATENCY".to_string(), self.config.buffer_size.to_string());
        env_vars.insert("PIPEWIRE_QUANTUM".to_string(), (self.config.buffer_size / 2).to_string());
        env_vars.insert("PIPEWIRE_RATE".to_string(), self.config.sample_rate.to_string());
    }

    async fn configure_pulseaudio_container(
        &self,
        env_vars: &mut HashMap<String, String>,
        volumes: &mut Vec<String>,
        paths: &AudioRuntimePaths,
    ) {
        info!("🔊 Configuring PulseAudio container audio");

        // PulseAudio environment variables
        if let Some(pulse_server) = &paths.pulse_server {
            env_vars.insert("PULSE_SERVER".to_string(), pulse_server.clone());
        }

        env_vars.insert("PULSE_RUNTIME_PATH".to_string(),
                       paths.runtime_dir.join("pulse").to_string_lossy().to_string());

        // PulseAudio socket mount
        volumes.push(format!("{}:/tmp/pulse-socket:rw",
                           paths.socket_path.to_string_lossy()));

        // PulseAudio-specific optimizations
        env_vars.insert("PULSE_LATENCY_MSEC".to_string(), "20".to_string()); // 20ms latency
    }

    async fn configure_alsa_container(
        &self,
        env_vars: &mut HashMap<String, String>,
        volumes: &mut Vec<String>,
        devices: &mut Vec<String>,
    ) {
        info!("🎶 Configuring ALSA container audio");

        // ALSA environment variables
        env_vars.insert("ALSA_RATE".to_string(), self.config.sample_rate.to_string());
        env_vars.insert("ALSA_CHANNELS".to_string(), "2".to_string());

        if let Some(device_name) = &self.config.device_name {
            env_vars.insert("ALSA_PCM_DEVICE".to_string(), device_name.clone());
        }

        // Mount ALSA devices
        devices.push("/dev/snd:/dev/snd:rw".to_string());

        // ALSA configuration
        volumes.push("/usr/share/alsa:/usr/share/alsa:ro".to_string());
    }

    async fn apply_gaming_audio_optimizations(&self, env_vars: &mut HashMap<String, String>) {
        info!("🎮 Applying gaming audio optimizations");

        // Gaming-specific audio settings
        env_vars.insert("AUDIO_GAMING_MODE".to_string(), "1".to_string());
        env_vars.insert("AUDIO_3D_PROCESSING".to_string(), "1".to_string());
        env_vars.insert("AUDIO_SURROUND_ENABLE".to_string(), "1".to_string());

        // Enable audio passthrough for gaming headsets
        if self.config.passthrough_enabled {
            env_vars.insert("AUDIO_PASSTHROUGH".to_string(), "1".to_string());
            env_vars.insert("AUDIO_EXCLUSIVE_MODE".to_string(), "1".to_string());
        }

        // Gaming audio profiles
        env_vars.insert("AUDIO_PROFILE".to_string(), "gaming".to_string());
        env_vars.insert("AUDIO_EQ_GAMING".to_string(), "1".to_string());
    }

    async fn apply_low_latency_optimizations(&self, env_vars: &mut HashMap<String, String>) {
        info!("⚡ Applying low-latency audio optimizations");

        // Low-latency buffer settings
        env_vars.insert("AUDIO_BUFFER_SIZE".to_string(), self.config.buffer_size.to_string());
        env_vars.insert("AUDIO_PERIODS".to_string(), "2".to_string()); // Minimize buffering

        // High-priority audio processing
        env_vars.insert("AUDIO_RT_PRIORITY".to_string(), "80".to_string());
        env_vars.insert("AUDIO_RT_SCHEDULING".to_string(), "1".to_string());

        // Disable audio processing that adds latency
        env_vars.insert("AUDIO_DISABLE_RESAMPLING".to_string(), "1".to_string());
        env_vars.insert("AUDIO_DISABLE_EFFECTS".to_string(), "1".to_string());
    }

    pub async fn get_audio_devices(&self) -> Vec<AudioDevice> {
        self.available_devices.read().await.clone()
    }

    pub async fn get_detected_subsystem(&self) -> AudioSubsystem {
        self.detected_subsystem.read().await.clone()
    }

    pub async fn get_optimal_settings_for_gaming(&self) -> AudioConfig {
        info!("🎯 Getting optimal audio settings for gaming");

        let subsystem = self.detected_subsystem.read().await;
        let mut config = self.config.clone();

        // Optimize based on detected subsystem
        match &*subsystem {
            AudioSubsystem::PipeWire => {
                config.buffer_size = 64; // Ultra-low latency with PipeWire
                config.sample_rate = 48000;
                config.bit_depth = 24;
            }
            AudioSubsystem::PulseAudio => {
                config.buffer_size = 128; // Reasonable latency with PulseAudio
                config.sample_rate = 48000;
                config.bit_depth = 16; // Lower CPU usage
            }
            AudioSubsystem::ALSA => {
                config.buffer_size = 64; // ALSA can go very low
                config.sample_rate = 48000;
                config.bit_depth = 24;
            }
            _ => {}
        }

        config.low_latency_mode = true;
        config.gaming_optimizations = true;
        config.passthrough_enabled = true;

        info!("✅ Optimal gaming audio: {}Hz, {}bit, {} buffer",
              config.sample_rate, config.bit_depth, config.buffer_size);

        config
    }

    pub async fn verify_audio_latency(&self, container_id: &str) -> Result<AudioLatencyMetrics> {
        info!("📊 Verifying audio latency for container: {}", container_id);

        let subsystem = self.detected_subsystem.read().await;

        // Estimate latency based on configuration and subsystem
        let estimated_latency_ms = match &*subsystem {
            AudioSubsystem::PipeWire => (self.config.buffer_size as f64 / self.config.sample_rate as f64) * 1000.0 + 2.0,
            AudioSubsystem::PulseAudio => (self.config.buffer_size as f64 / self.config.sample_rate as f64) * 1000.0 + 5.0,
            AudioSubsystem::ALSA => (self.config.buffer_size as f64 / self.config.sample_rate as f64) * 1000.0 + 1.0,
            _ => 50.0, // Conservative estimate
        };

        let metrics = AudioLatencyMetrics {
            subsystem: subsystem.clone(),
            estimated_latency_ms,
            buffer_size: self.config.buffer_size,
            sample_rate: self.config.sample_rate,
            bit_depth: self.config.bit_depth,
            is_gaming_optimized: self.config.gaming_optimizations,
            is_low_latency_mode: self.config.low_latency_mode,
        };

        info!("📈 Audio Latency Metrics:");
        info!("   Subsystem: {:?}", metrics.subsystem);
        info!("   Estimated Latency: {:.1}ms", metrics.estimated_latency_ms);
        info!("   Buffer: {} samples @ {}Hz", metrics.buffer_size, metrics.sample_rate);
        info!("   Gaming Optimized: {}", metrics.is_gaming_optimized);

        // Check if latency target is met
        let target_latency_ms = if self.config.gaming_optimizations { 10.0 } else { 20.0 };
        if estimated_latency_ms <= target_latency_ms {
            info!("✅ Audio latency target achieved: {:.1}ms <= {:.1}ms", estimated_latency_ms, target_latency_ms);
        } else {
            warn!("⚠️  Audio latency target missed: {:.1}ms > {:.1}ms", estimated_latency_ms, target_latency_ms);
        }

        Ok(metrics)
    }
}

#[derive(Debug, Clone)]
pub struct AudioContainerConfig {
    pub env_vars: HashMap<String, String>,
    pub volumes: Vec<String>,
    pub devices: Vec<String>,
    pub subsystem: AudioSubsystem,
}

#[derive(Debug, Clone)]
pub struct AudioLatencyMetrics {
    pub subsystem: AudioSubsystem,
    pub estimated_latency_ms: f64,
    pub buffer_size: u32,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub is_gaming_optimized: bool,
    pub is_low_latency_mode: bool,
}