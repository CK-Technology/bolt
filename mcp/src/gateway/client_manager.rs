//! Client Manager
//!
//! Tracks connected MCP clients

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Client Manager
///
/// Manages connected MCP clients
pub struct ClientManager {
    clients: Arc<DashMap<Uuid, ClientInfo>>,
}

/// Client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client ID
    pub id: Uuid,

    /// Client name
    pub name: String,

    /// Transport type
    pub transport: String,

    /// Connected at
    pub connected_at: DateTime<Utc>,

    /// Last activity
    pub last_activity: DateTime<Utc>,

    /// Request count
    pub request_count: u64,
}

impl ClientManager {
    /// Create a new client manager
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
        }
    }

    /// Register a new client
    pub fn register_client(&self, name: String, transport: String) -> Uuid {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let client = ClientInfo {
            id,
            name,
            transport,
            connected_at: now,
            last_activity: now,
            request_count: 0,
        };

        self.clients.insert(id, client);
        id
    }

    /// Unregister a client
    pub fn unregister_client(&self, id: &Uuid) -> Option<ClientInfo> {
        self.clients.remove(id).map(|(_, client)| client)
    }

    /// Update client activity
    pub fn update_activity(&self, id: &Uuid) {
        if let Some(mut client) = self.clients.get_mut(id) {
            client.last_activity = Utc::now();
            client.request_count += 1;
        }
    }

    /// Get client info
    pub fn get_client(&self, id: &Uuid) -> Option<ClientInfo> {
        self.clients.get(id).map(|entry| entry.value().clone())
    }

    /// Get all clients
    pub fn get_all_clients(&self) -> Vec<ClientInfo> {
        self.clients
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get client count
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Clear all clients
    pub fn clear(&self) {
        self.clients.clear();
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_client() {
        let manager = ClientManager::new();
        let id = manager.register_client("test-client".to_string(), "websocket".to_string());

        assert_eq!(manager.client_count(), 1);
        let client = manager.get_client(&id).unwrap();
        assert_eq!(client.name, "test-client");
        assert_eq!(client.transport, "websocket");
    }

    #[test]
    fn test_unregister_client() {
        let manager = ClientManager::new();
        let id = manager.register_client("test-client".to_string(), "websocket".to_string());

        assert_eq!(manager.client_count(), 1);

        let client = manager.unregister_client(&id);
        assert!(client.is_some());
        assert_eq!(manager.client_count(), 0);
    }

    #[test]
    fn test_update_activity() {
        let manager = ClientManager::new();
        let id = manager.register_client("test-client".to_string(), "websocket".to_string());

        let client_before = manager.get_client(&id).unwrap();
        assert_eq!(client_before.request_count, 0);

        manager.update_activity(&id);

        let client_after = manager.get_client(&id).unwrap();
        assert_eq!(client_after.request_count, 1);
        assert!(client_after.last_activity > client_before.last_activity);
    }
}
