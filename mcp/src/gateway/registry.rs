//! Tool Registry
//!
//! Manages enabled/disabled tools across MCP servers

use dashmap::DashMap;
use std::sync::Arc;

/// Tool Registry
///
/// Tracks which tools are enabled/disabled for each server
pub struct ToolRegistry {
    /// Map of server:tool -> enabled
    tools: Arc<DashMap<String, bool>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(DashMap::new()),
        }
    }

    /// Register a tool
    pub fn register_tool(&self, server: &str, tool: &str, enabled: bool) {
        let key = format!("{}:{}", server, tool);
        self.tools.insert(key, enabled);
    }

    /// Enable a tool
    pub fn enable_tool(&self, server: &str, tool: &str) {
        let key = format!("{}:{}", server, tool);
        self.tools.insert(key, true);
    }

    /// Disable a tool
    pub fn disable_tool(&self, server: &str, tool: &str) {
        let key = format!("{}:{}", server, tool);
        self.tools.insert(key, false);
    }

    /// Check if a tool is enabled
    pub fn is_tool_enabled(&self, server: &str, tool: &str) -> bool {
        let key = format!("{}:{}", server, tool);
        self.tools.get(&key).map(|v| *v).unwrap_or(true)
    }

    /// Get all enabled tools for a server
    pub fn get_enabled_tools(&self, server: &str) -> Vec<String> {
        let prefix = format!("{}:", server);
        self.tools
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix) && *entry.value())
            .map(|entry| {
                entry
                    .key()
                    .strip_prefix(&prefix)
                    .unwrap_or(entry.key())
                    .to_string()
            })
            .collect()
    }

    /// Get total tool count
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Clear all tools
    pub fn clear(&self) {
        self.tools.clear();
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_tool() {
        let registry = ToolRegistry::new();
        registry.register_tool("server1", "tool1", true);
        assert!(registry.is_tool_enabled("server1", "tool1"));
    }

    #[test]
    fn test_enable_disable_tool() {
        let registry = ToolRegistry::new();
        registry.register_tool("server1", "tool1", true);
        assert!(registry.is_tool_enabled("server1", "tool1"));

        registry.disable_tool("server1", "tool1");
        assert!(!registry.is_tool_enabled("server1", "tool1"));

        registry.enable_tool("server1", "tool1");
        assert!(registry.is_tool_enabled("server1", "tool1"));
    }

    #[test]
    fn test_get_enabled_tools() {
        let registry = ToolRegistry::new();
        registry.register_tool("server1", "tool1", true);
        registry.register_tool("server1", "tool2", false);
        registry.register_tool("server1", "tool3", true);

        let enabled = registry.get_enabled_tools("server1");
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&"tool1".to_string()));
        assert!(enabled.contains(&"tool3".to_string()));
    }
}
