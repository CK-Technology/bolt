//! Intel Quick Sync Video Acceleration Integration
//!
//! Provides hardware video encoding/decoding support for media containers
//! via VA-API (Video Acceleration API) on Linux.
//!
//! Quick Sync enables:
//! - H.264/HEVC decode: 5-10x faster than software
//! - H.264/AV1 encode: 5-10x faster, 80% lower power
//! - Transcoding: Near-zero CPU usage for media servers

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Intel Quick Sync configuration for containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickSyncConfig {
    pub enabled: bool,
    pub device_path: PathBuf,         // /dev/dri/renderD128
    pub vaapi_driver: Option<String>, // iHD (newer) or i965 (legacy)
    pub codecs: Vec<VideoCodec>,
    pub quality_preset: QualityPreset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VideoCodec {
    H264,
    HEVC,
    VP9,
    AV1,
    MPEG2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityPreset {
    Speed,    // Fastest encode, lower quality
    Balanced, // Good balance
    Quality,  // Best quality, slower
}

/// Quick Sync device information
#[derive(Debug, Clone)]
pub struct QuickSyncDevice {
    pub render_node: PathBuf,
    pub card_path: PathBuf,
    pub pci_id: Option<String>,
    pub intel_generation: IntelGeneration,
    pub capabilities: QuickSyncCaps,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntelGeneration {
    Gen9,  // Skylake, Kaby Lake
    Gen11, // Ice Lake
    Gen12, // Tiger Lake, Alder Lake, Raptor Lake
    Arc,   // Alchemist (A-series discrete)
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct QuickSyncCaps {
    pub h264_decode: bool,
    pub h264_encode: bool,
    pub hevc_decode: bool,
    pub hevc_encode: bool,
    pub vp9_decode: bool,
    pub av1_decode: bool,
    pub av1_encode: bool, // Gen12+ / Arc only
    pub max_resolution: String,
}

impl QuickSyncDevice {
    /// Detect Intel GPU with Quick Sync support
    pub fn detect() -> Result<Option<Self>> {
        let drm_path = Path::new("/sys/class/drm");
        if !drm_path.exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(drm_path)?.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Look for Intel card devices
            if !name_str.starts_with("card") || name_str.contains('-') {
                continue;
            }

            // Check vendor ID (0x8086 = Intel)
            let vendor_path = entry.path().join("device/vendor");
            let vendor = fs::read_to_string(&vendor_path).ok();
            if vendor.as_ref().map(|v| v.trim()) != Some("0x8086") {
                continue;
            }

            // Get device ID to determine generation
            let device_id_path = entry.path().join("device/device");
            let device_id = fs::read_to_string(&device_id_path)
                .ok()
                .and_then(|d| d.trim().strip_prefix("0x").map(|s| s.to_string()));

            let intel_generation = device_id
                .as_ref()
                .map(|id| Self::determine_generation(id))
                .unwrap_or(IntelGeneration::Unknown);

            let card_path = PathBuf::from(format!("/dev/dri/{}", name_str));
            if !card_path.exists() {
                continue;
            }

            // Find corresponding render node
            let idx = name_str.trim_start_matches("card").parse::<u32>().ok();
            let render_node = idx
                .map(|i| PathBuf::from(format!("/dev/dri/renderD{}", 128 + i)))
                .filter(|p| p.exists());

            if render_node.is_none() {
                warn!("Intel GPU found but no render node available for Quick Sync");
                continue;
            }

            let pci_id = fs::read_to_string(entry.path().join("device/uevent"))
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find_map(|line| line.strip_prefix("PCI_SLOT_NAME="))
                        .map(|s| s.trim().to_string())
                });

            let capabilities = Self::detect_capabilities(&intel_generation);

            info!(
                "✅ Intel Quick Sync detected: {:?} generation",
                intel_generation
            );
            info!(
                "   Render node: {}",
                render_node.as_ref().unwrap().display()
            );
            if capabilities.av1_encode {
                info!("   🎬 AV1 hardware encode available (Arc/Gen12+)");
            }

            return Ok(Some(Self {
                render_node: render_node.unwrap(),
                card_path,
                pci_id,
                intel_generation,
                capabilities,
            }));
        }

        Ok(None)
    }

    fn determine_generation(device_id: &str) -> IntelGeneration {
        // Intel device ID ranges (approximate)
        match device_id.chars().next() {
            Some('5') if device_id.starts_with("56") => IntelGeneration::Arc, // Arc A-series: 56xx
            Some('4') | Some('9') => IntelGeneration::Gen12, // Tiger Lake+: 9axx, 46xx
            Some('8') => IntelGeneration::Gen11,             // Ice Lake: 8axx
            Some('5') | Some('3') | Some('1') | Some('0') if device_id.len() == 4 => {
                IntelGeneration::Gen9 // Skylake-Kaby: 590x, 591x, 3exx
            }
            _ => IntelGeneration::Unknown,
        }
    }

    fn detect_capabilities(generation: &IntelGeneration) -> QuickSyncCaps {
        match generation {
            IntelGeneration::Arc | IntelGeneration::Gen12 => QuickSyncCaps {
                h264_decode: true,
                h264_encode: true,
                hevc_decode: true,
                hevc_encode: true,
                vp9_decode: true,
                av1_decode: true,
                av1_encode: generation == &IntelGeneration::Arc, // Only Arc has AV1 encode
                max_resolution: "8K".to_string(),
            },
            IntelGeneration::Gen11 => QuickSyncCaps {
                h264_decode: true,
                h264_encode: true,
                hevc_decode: true,
                hevc_encode: true,
                vp9_decode: true,
                av1_decode: false,
                av1_encode: false,
                max_resolution: "4K".to_string(),
            },
            IntelGeneration::Gen9 => QuickSyncCaps {
                h264_decode: true,
                h264_encode: true,
                hevc_decode: true,
                hevc_encode: true,
                vp9_decode: false,
                av1_decode: false,
                av1_encode: false,
                max_resolution: "4K".to_string(),
            },
            IntelGeneration::Unknown => QuickSyncCaps::default(),
        }
    }

    /// Get environment variables for VA-API in containers
    pub fn vaapi_env_vars(&self) -> Vec<String> {
        vec![
            format!("LIBVA_DRIVER_NAME={}", self.vaapi_driver()),
            format!("LIBVA_DRIVERS_PATH=/usr/lib/x86_64-linux-gnu/dri"),
            format!("LIBVA_MESSAGING_LEVEL=1"), // Info level
        ]
    }

    /// Determine best VA-API driver for this generation
    pub fn vaapi_driver(&self) -> String {
        match self.intel_generation {
            IntelGeneration::Arc | IntelGeneration::Gen12 | IntelGeneration::Gen11 => {
                "iHD".to_string() // Modern driver
            }
            IntelGeneration::Gen9 | IntelGeneration::Unknown => {
                "i965".to_string() // Legacy driver
            }
        }
    }

    /// Get OCI device mounts for Quick Sync
    pub fn oci_device_node(&self) -> (PathBuf, Option<u32>, Option<u32>) {
        use nix::sys::stat::stat;

        // Get device major/minor numbers
        let stat_result = stat(&self.render_node).ok();
        let (major, minor) = stat_result
            .map(|st| {
                let major = nix::sys::stat::major(st.st_rdev) as u32;
                let minor = nix::sys::stat::minor(st.st_rdev) as u32;
                (Some(major), Some(minor))
            })
            .unwrap_or((None, None));

        (self.render_node.clone(), major, minor)
    }

    /// Generate FFmpeg flags for hardware acceleration
    pub fn ffmpeg_hw_flags(&self, codec: &VideoCodec) -> Vec<String> {
        let mut flags = vec![
            "-hwaccel".to_string(),
            "vaapi".to_string(),
            "-hwaccel_device".to_string(),
            self.render_node.to_string_lossy().to_string(),
            "-hwaccel_output_format".to_string(),
            "vaapi".to_string(),
        ];

        // Add codec-specific flags
        match codec {
            VideoCodec::H264 if self.capabilities.h264_encode => {
                flags.extend(vec!["-c:v".to_string(), "h264_vaapi".to_string()]);
            }
            VideoCodec::HEVC if self.capabilities.hevc_encode => {
                flags.extend(vec!["-c:v".to_string(), "hevc_vaapi".to_string()]);
            }
            VideoCodec::AV1 if self.capabilities.av1_encode => {
                flags.extend(vec!["-c:v".to_string(), "av1_vaapi".to_string()]);
            }
            _ => {}
        }

        flags
    }
}

impl QuickSyncConfig {
    /// Create config from detected device
    pub fn from_device(device: &QuickSyncDevice) -> Self {
        let codecs = vec![
            VideoCodec::H264,
            VideoCodec::HEVC,
            VideoCodec::VP9,
            VideoCodec::AV1,
        ]
        .into_iter()
        .filter(|codec| match codec {
            VideoCodec::H264 => device.capabilities.h264_decode || device.capabilities.h264_encode,
            VideoCodec::HEVC => device.capabilities.hevc_decode || device.capabilities.hevc_encode,
            VideoCodec::VP9 => device.capabilities.vp9_decode,
            VideoCodec::AV1 => device.capabilities.av1_decode || device.capabilities.av1_encode,
            _ => false,
        })
        .collect();

        Self {
            enabled: true,
            device_path: device.render_node.clone(),
            vaapi_driver: Some(device.vaapi_driver()),
            codecs,
            quality_preset: QualityPreset::Balanced,
        }
    }

    /// Auto-detect and create config if Quick Sync is available
    pub fn auto_detect() -> Result<Option<Self>> {
        match QuickSyncDevice::detect()? {
            Some(device) => Ok(Some(Self::from_device(&device))),
            None => Ok(None),
        }
    }
}

/// Container hints for Quick Sync optimization
#[derive(Debug, Clone)]
pub enum MediaWorkload {
    Transcoding, // Plex, Jellyfin, Emby
    Streaming,   // OBS, live streaming
    Recording,   // Video capture
    Playback,    // Video player
}

impl MediaWorkload {
    /// Get recommended Quick Sync settings for this workload
    pub fn recommended_preset(&self) -> QualityPreset {
        match self {
            MediaWorkload::Transcoding => QualityPreset::Balanced,
            MediaWorkload::Streaming => QualityPreset::Speed,
            MediaWorkload::Recording => QualityPreset::Quality,
            MediaWorkload::Playback => QualityPreset::Speed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_sync_detection() {
        // Detection should not panic
        let _result = QuickSyncDevice::detect();
    }

    #[test]
    fn test_intel_generation_detection() {
        assert_eq!(
            QuickSyncDevice::determine_generation("5690"),
            IntelGeneration::Arc
        );
        assert_eq!(
            QuickSyncDevice::determine_generation("9a49"),
            IntelGeneration::Gen12
        );
    }

    #[test]
    fn test_vaapi_driver_selection() {
        let arc_dev = QuickSyncDevice {
            render_node: PathBuf::from("/dev/dri/renderD128"),
            card_path: PathBuf::from("/dev/dri/card0"),
            pci_id: None,
            intel_generation: IntelGeneration::Arc,
            capabilities: QuickSyncCaps::default(),
        };

        assert_eq!(arc_dev.vaapi_driver(), "iHD");

        let gen9_dev = QuickSyncDevice {
            intel_generation: IntelGeneration::Gen9,
            ..arc_dev
        };

        assert_eq!(gen9_dev.vaapi_driver(), "i965");
    }
}
