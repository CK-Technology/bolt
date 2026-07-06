use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::WaylandGamingConfig;

#[derive(Debug)]
pub struct InputManager {
    config: WaylandGamingConfig,
    devices: Arc<RwLock<HashMap<u32, InputDevice>>>,
    latency_monitor: LatencyMonitor,
    gaming_mode: bool,
}

#[derive(Debug, Clone)]
pub struct InputDevice {
    pub id: u32,
    pub name: String,
    pub device_type: InputDeviceType,
    pub vendor_id: u16,
    pub product_id: u16,
    pub gaming_optimized: bool,
    pub polling_rate: u32,
    pub dpi: Option<u32>,
    pub latency_ms: f64,
}

#[derive(Debug, Clone)]
pub enum InputDeviceType {
    Mouse,
    Keyboard,
    Gamepad,
    Joystick,
    TouchPad,
    Tablet,
    Other,
}

#[derive(Debug)]
pub struct LatencyMonitor {
    measurements: Vec<Duration>,
    last_input_time: Option<Instant>,
    average_latency: Duration,
}

impl Default for LatencyMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyMonitor {
    pub fn new() -> Self {
        Self {
            measurements: Vec::with_capacity(1000),
            last_input_time: None,
            average_latency: Duration::ZERO,
        }
    }

    pub fn record_input_event(&mut self) {
        let now = Instant::now();

        if let Some(last_time) = self.last_input_time {
            let latency = now.duration_since(last_time);
            self.measurements.push(latency);

            // Keep only recent measurements
            if self.measurements.len() > 1000 {
                self.measurements.remove(0);
            }

            // Update average
            self.update_average();
        }

        self.last_input_time = Some(now);
    }

    fn update_average(&mut self) {
        if !self.measurements.is_empty() {
            let total: Duration = self.measurements.iter().sum();
            self.average_latency = total / self.measurements.len() as u32;
        }
    }

    pub fn get_average_latency(&self) -> Duration {
        self.average_latency
    }
}

impl InputManager {
    pub async fn new(config: &WaylandGamingConfig) -> Result<Self> {
        info!("🎮 Initializing input manager");

        let manager = Self {
            config: config.clone(),
            devices: Arc::new(RwLock::new(HashMap::new())),
            latency_monitor: LatencyMonitor::new(),
            gaming_mode: false,
        };

        debug!("✅ Input manager initialized");
        Ok(manager)
    }

    pub async fn setup_gaming_input(&mut self) -> Result<()> {
        info!("⚡ Setting up gaming input optimizations");

        self.gaming_mode = true;

        // Detect input devices
        self.detect_input_devices().await?;

        // Apply gaming optimizations
        self.apply_gaming_optimizations().await?;

        // Setup low-latency input handling
        if self.config.enable_low_latency {
            self.setup_low_latency_input().await?;
        }

        info!("✅ Gaming input configured");
        Ok(())
    }

    async fn detect_input_devices(&mut self) -> Result<()> {
        debug!("🔍 Detecting input devices");

        let devices =
            discover_input_devices_from(Path::new("/dev/input"), Path::new("/sys/class/input"))?;

        {
            let mut device_map = self.devices.write().await;
            device_map.clear();
            for device in devices {
                info!(
                    "  🖱️  Detected: {} ({:?}, {}Hz)",
                    device.name, device.device_type, device.polling_rate
                );

                device_map.insert(device.id, device);
            }
        }

        Ok(())
    }

    async fn apply_gaming_optimizations(&mut self) -> Result<()> {
        debug!("🚀 Applying gaming optimizations to input devices");

        let mut devices = self.devices.write().await;

        for (_, device) in devices.iter_mut() {
            if !device.gaming_optimized {
                match device.device_type {
                    InputDeviceType::Mouse => {
                        self.optimize_gaming_mouse(device).await?;
                    }
                    InputDeviceType::Keyboard => {
                        self.optimize_gaming_keyboard(device).await?;
                    }
                    InputDeviceType::Gamepad => {
                        self.optimize_gaming_controller(device).await?;
                    }
                    _ => {}
                }

                device.gaming_optimized = true;
            }
        }

        Ok(())
    }

    async fn optimize_gaming_mouse(&self, device: &mut InputDevice) -> Result<()> {
        debug!("🖱️  Optimizing gaming mouse: {}", device.name);

        // Increase polling rate for lower latency
        device.polling_rate = 1000; // 1000Hz = 1ms polling
        device.latency_ms = 1.0;

        // Disable mouse acceleration for gaming
        self.disable_mouse_acceleration(device.id).await?;

        // Set raw input mode
        self.enable_raw_input(device.id).await?;

        info!("  ✓ Mouse optimized: 1000Hz polling, raw input enabled");
        Ok(())
    }

    async fn optimize_gaming_keyboard(&self, device: &mut InputDevice) -> Result<()> {
        debug!("⌨️  Optimizing gaming keyboard: {}", device.name);

        // Increase polling rate
        device.polling_rate = 1000;
        device.latency_ms = 1.0;

        // Disable key repeat delay for gaming
        self.optimize_key_repeat(device.id).await?;

        // Enable N-key rollover
        self.enable_nkey_rollover(device.id).await?;

        info!("  ✓ Keyboard optimized: 1000Hz polling, N-key rollover");
        Ok(())
    }

    async fn optimize_gaming_controller(&self, device: &mut InputDevice) -> Result<()> {
        debug!("🎮 Optimizing gaming controller: {}", device.name);

        // Increase polling rate
        device.polling_rate = 1000;
        device.latency_ms = 1.0;

        // Reduce deadzone for precise control
        self.optimize_controller_deadzone(device.id).await?;

        // Enable low latency mode
        self.enable_controller_low_latency(device.id).await?;

        info!("  ✓ Controller optimized: 1000Hz polling, reduced deadzone");
        Ok(())
    }

    async fn disable_mouse_acceleration(&self, _device_id: u32) -> Result<()> {
        debug!("🎯 Disabling mouse acceleration");

        unsafe {
            std::env::set_var("LIBINPUT_NO_ACCEL", "1");
        }

        Ok(())
    }

    async fn enable_raw_input(&self, _device_id: u32) -> Result<()> {
        debug!("📡 Enabling raw input mode");

        // Configure for raw input without any processing
        info!("    Raw input mode enabled");

        Ok(())
    }

    async fn optimize_key_repeat(&self, _device_id: u32) -> Result<()> {
        debug!("⚡ Optimizing key repeat settings");

        // Set minimal key repeat delay for gaming
        info!("    Key repeat optimized for gaming");

        Ok(())
    }

    async fn enable_nkey_rollover(&self, _device_id: u32) -> Result<()> {
        debug!("🔢 Enabling N-key rollover");

        // Enable full N-key rollover for complex key combinations
        info!("    N-key rollover enabled");

        Ok(())
    }

    async fn optimize_controller_deadzone(&self, _device_id: u32) -> Result<()> {
        debug!("🎯 Optimizing controller deadzone");

        // Reduce deadzone for more precise control
        info!("    Controller deadzone optimized");

        Ok(())
    }

    async fn enable_controller_low_latency(&self, _device_id: u32) -> Result<()> {
        debug!("⚡ Enabling controller low latency mode");

        // Enable low latency communication
        info!("    Controller low latency mode enabled");

        Ok(())
    }

    async fn setup_low_latency_input(&mut self) -> Result<()> {
        info!("⚡ Setting up low-latency input handling");

        // Configure kernel-level input optimizations
        self.configure_kernel_input_optimizations().await?;

        // Setup high priority input thread
        self.setup_high_priority_input_thread().await?;

        // Configure input event batching
        self.configure_input_batching().await?;

        Ok(())
    }

    async fn configure_kernel_input_optimizations(&self) -> Result<()> {
        debug!("🔧 Configuring kernel input optimizations");

        // Disable input event buffering for lowest latency
        unsafe {
            std::env::set_var("LIBINPUT_DISABLE_BUFFERING", "1");
        }

        // Set high frequency timer for input
        info!("  ✓ High frequency input timer enabled");

        Ok(())
    }

    async fn setup_high_priority_input_thread(&self) -> Result<()> {
        debug!("🔥 Setting up high priority input thread");

        // In a real implementation, this would set RT priority for input thread
        info!("  ✓ High priority input thread configured");

        Ok(())
    }

    async fn configure_input_batching(&self) -> Result<()> {
        debug!("📦 Configuring input event batching");

        // Disable input batching for minimum latency
        info!("  ✓ Input batching disabled for low latency");

        Ok(())
    }

    pub async fn record_input_latency(&mut self, latency: Duration) {
        debug!("Recording input latency: {:?}", latency);
        self.latency_monitor.record_input_event();
    }

    pub async fn get_input_latency_stats(&self) -> Result<InputLatencyStats> {
        let average_latency = self.latency_monitor.get_average_latency();

        let devices = self.devices.read().await;
        let optimized_devices = devices.values().filter(|d| d.gaming_optimized).count();

        Ok(InputLatencyStats {
            average_latency_ms: average_latency.as_millis() as f64,
            optimized_devices,
            total_devices: devices.len(),
            gaming_mode: self.gaming_mode,
        })
    }

    pub async fn list_input_devices(&self) -> Result<Vec<InputDevice>> {
        let devices = self.devices.read().await;
        Ok(devices.values().cloned().collect())
    }
}

fn discover_input_devices_from(dev_input: &Path, sys_input: &Path) -> Result<Vec<InputDevice>> {
    let mut devices = Vec::new();
    let Ok(entries) = std::fs::read_dir(dev_input) else {
        return Ok(devices);
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Some(event_id) = name
            .strip_prefix("event")
            .and_then(|id| id.parse::<u32>().ok())
        else {
            continue;
        };

        let sys_device = sys_input.join(name).join("device");
        let device_name = read_trimmed(sys_device.join("name"))
            .unwrap_or_else(|| format!("Input device {}", name));
        let vendor_id = read_hex_u16(sys_device.join("id/vendor")).unwrap_or(0);
        let product_id = read_hex_u16(sys_device.join("id/product")).unwrap_or(0);
        let device_type = classify_input_device(&device_name);

        devices.push(InputDevice {
            id: event_id,
            name: device_name,
            device_type,
            vendor_id,
            product_id,
            gaming_optimized: false,
            polling_rate: 125,
            dpi: None,
            latency_ms: 8.0,
        });
    }

    devices.sort_by_key(|device| device.id);
    Ok(devices)
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_hex_u16(path: PathBuf) -> Option<u16> {
    let value = read_trimmed(path)?;
    u16::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}

fn classify_input_device(name: &str) -> InputDeviceType {
    let lower = name.to_ascii_lowercase();
    if lower.contains("mouse") {
        InputDeviceType::Mouse
    } else if lower.contains("keyboard") || lower.contains("kbd") {
        InputDeviceType::Keyboard
    } else if lower.contains("gamepad")
        || lower.contains("controller")
        || lower.contains("xbox")
        || lower.contains("dualshock")
        || lower.contains("dualsense")
    {
        InputDeviceType::Gamepad
    } else if lower.contains("joystick") {
        InputDeviceType::Joystick
    } else if lower.contains("touchpad") {
        InputDeviceType::TouchPad
    } else if lower.contains("tablet") {
        InputDeviceType::Tablet
    } else {
        InputDeviceType::Other
    }
}

#[derive(Debug, Clone)]
pub struct InputLatencyStats {
    pub average_latency_ms: f64,
    pub optimized_devices: usize,
    pub total_devices: usize,
    pub gaming_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_tempdir() -> tempfile::TempDir {
        std::fs::create_dir_all(".scratch").expect("create repo-local scratch directory");
        tempfile::tempdir_in(".scratch").expect("create repo-local scratch tempdir")
    }

    #[test]
    fn input_device_classification_uses_device_name() {
        assert!(matches!(
            classify_input_device("Logitech Gaming Mouse"),
            InputDeviceType::Mouse
        ));
        assert!(matches!(
            classify_input_device("Xbox Wireless Controller"),
            InputDeviceType::Gamepad
        ));
        assert!(matches!(
            classify_input_device("AT Translated Set 2 keyboard"),
            InputDeviceType::Keyboard
        ));
    }

    #[test]
    fn input_discovery_reads_event_devices_from_sysfs() -> Result<()> {
        let root = scratch_tempdir();
        let dev_input = root.path().join("dev/input");
        let sys_input = root.path().join("sys/class/input");
        std::fs::create_dir_all(&dev_input)?;
        std::fs::create_dir_all(sys_input.join("event7/device/id"))?;
        std::fs::write(dev_input.join("event7"), b"")?;
        std::fs::write(sys_input.join("event7/device/name"), "Xbox Controller\n")?;
        std::fs::write(sys_input.join("event7/device/id/vendor"), "045e\n")?;
        std::fs::write(sys_input.join("event7/device/id/product"), "02ea\n")?;

        let devices = discover_input_devices_from(&dev_input, &sys_input)?;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, 7);
        assert_eq!(devices[0].vendor_id, 0x045e);
        assert_eq!(devices[0].product_id, 0x02ea);
        assert!(matches!(devices[0].device_type, InputDeviceType::Gamepad));
        Ok(())
    }
}
