use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Container Device Interface (CDI) specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDISpec {
    #[serde(rename = "cdiVersion")]
    pub cdi_version: String,
    pub kind: String,
    pub devices: Vec<CDIDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDIDevice {
    pub name: String,
    #[serde(rename = "containerEdits")]
    pub container_edits: CDIContainerEdits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDIContainerEdits {
    #[serde(rename = "deviceNodes")]
    pub device_nodes: Vec<CDIDeviceNode>,
    pub mounts: Vec<CDIMount>,
    pub env: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDIDeviceNode {
    pub path: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub major: u32,
    pub minor: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDIMount {
    #[serde(rename = "hostPath")]
    pub host_path: String,
    #[serde(rename = "containerPath")]
    pub container_path: String,
    pub options: Vec<String>,
}

/// AMD GPU information structure
#[derive(Debug, Clone)]
pub struct AMDGPUInfo {
    pub name: String,
    pub memory_mb: u32,
    pub opencl_support: bool,
    pub rocm_support: bool,
    pub vulkan_support: bool,
    pub device_id: String,
}
