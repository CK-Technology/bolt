//! Native Bolt service tools.
//!
//! These are Bolt-owned tool capabilities that can be enabled from Boltfile.toml.
//! They provide docker-mcp-like ergonomics without requiring a separate sidecar,
//! crate, or external protocol server.

use crate::config::{Service, ServiceToolsConfig, ToolPermissions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTool {
    pub name: &'static str,
    pub description: &'static str,
    pub permission_scope: &'static str,
}

pub const BUILTIN_TOOLS: &[BuiltinTool] = &[
    BuiltinTool {
        name: "fs.read",
        description: "Read files from allowed service filesystem roots",
        permission_scope: "filesystem_roots",
    },
    BuiltinTool {
        name: "fs.write",
        description: "Write files under allowed service filesystem roots",
        permission_scope: "filesystem_roots",
    },
    BuiltinTool {
        name: "fs.list",
        description: "List files under allowed service filesystem roots",
        permission_scope: "filesystem_roots",
    },
    BuiltinTool {
        name: "fs.watch",
        description: "Watch allowed service filesystem roots for changes",
        permission_scope: "filesystem_roots",
    },
    BuiltinTool {
        name: "shell.exec",
        description: "Execute allow-listed commands in a service context",
        permission_scope: "shell_commands",
    },
    BuiltinTool {
        name: "gpu.stats",
        description: "Read GPU utilization and memory metrics",
        permission_scope: "gpu_access",
    },
    BuiltinTool {
        name: "gpu.info",
        description: "Read GPU inventory and driver information",
        permission_scope: "gpu_access",
    },
    BuiltinTool {
        name: "process.list",
        description: "List service processes",
        permission_scope: "process_access",
    },
    BuiltinTool {
        name: "process.kill",
        description: "Terminate service processes when explicitly allowed",
        permission_scope: "process_access",
    },
    BuiltinTool {
        name: "network.stats",
        description: "Read service network counters",
        permission_scope: "network_access",
    },
    BuiltinTool {
        name: "network.connections",
        description: "List service network connections",
        permission_scope: "network_access",
    },
];

pub fn all_tools() -> &'static [BuiltinTool] {
    BUILTIN_TOOLS
}

pub fn tool_by_name(name: &str) -> Option<&'static BuiltinTool> {
    BUILTIN_TOOLS.iter().find(|tool| tool.name == name)
}

pub fn enabled_tools_for_service(service: &Service) -> Vec<&'static BuiltinTool> {
    let Some(config) = service.tools.as_ref() else {
        return Vec::new();
    };

    if !config.enabled {
        return Vec::new();
    }

    config
        .allow
        .iter()
        .filter_map(|name| tool_by_name(name))
        .collect()
}

pub fn permissions_for_service(service: &Service) -> Option<&ToolPermissions> {
    service.tools.as_ref()?.permissions.as_ref()
}

pub fn validate_service_tools(config: &ServiceToolsConfig) -> crate::Result<()> {
    config.validate().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Service, ServiceToolsConfig};

    #[test]
    fn maps_enabled_tool_names_to_registry_entries() {
        let service = Service {
            tools: Some(ServiceToolsConfig {
                enabled: true,
                allow: vec!["fs.read".to_string(), "gpu.stats".to_string()],
                permissions: None,
            }),
            ..Default::default()
        };

        let tools = enabled_tools_for_service(&service);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "fs.read");
        assert_eq!(tools[1].name, "gpu.stats");
    }

    #[test]
    fn disabled_service_tools_return_empty_registry() {
        let service = Service {
            tools: Some(ServiceToolsConfig {
                enabled: false,
                allow: vec!["fs.read".to_string()],
                permissions: None,
            }),
            ..Default::default()
        };

        assert!(enabled_tools_for_service(&service).is_empty());
    }
}
