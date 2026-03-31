use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use uuid::Uuid;

/// A channel-based message for sending text to a specific WebSocket client
pub struct SendText {
    pub recipient_id: Uuid,
    pub message: String,
    pub sender_id: Uuid,
}

/// A channel-based message for broadcasting to all WebSocket clients
pub struct SendMessageToAll {
    pub message: String,
    pub sender_id: Uuid,
}

/// A channel-based message for closing a WebSocket connection
pub struct Close {
    pub id: Uuid,
}

/// Connection registry that maps client IDs to their message senders
pub struct ConnectionRegistry {
    clients: HashMap<Uuid, mpsc::Sender<String>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn register(&mut self, id: Uuid, tx: mpsc::Sender<String>) {
        self.clients.insert(id, tx);
    }

    pub fn unregister(&mut self, id: Uuid) {
        self.clients.remove(&id);
    }

    pub async fn send_text(&self, msg: SendText) {
        if let Some(tx) = self.clients.get(&msg.recipient_id) {
            if msg.recipient_id == msg.sender_id {
                // Don't send back to sender
                return;
            }
            let _ = tx.send(msg.message).await;
        } else {
            log::warn!("No client found for id: {}", msg.recipient_id);
        }
    }

    pub async fn broadcast(&self, msg: SendMessageToAll) {
        for (id, tx) in &self.clients {
            if *id != msg.sender_id {
                let _ = tx.send(msg.message.clone()).await;
            }
        }
    }

    pub async fn close_connection(&mut self, msg: Close) {
        if let Some(tx) = self.clients.remove(&msg.id) {
            // Send close signal to the client
            let _ = tx.send("Connection closed".to_string()).await;
        }
    }
}

pub type SharedRegistry = Arc<Mutex<ConnectionRegistry>>;

/// Global registry management for different WebSocket endpoints
static REGISTRY_ADDRESSES: once_cell::sync::OnceCell<
    parking_lot::RwLock<HashMap<String, SharedRegistry>>,
> = once_cell::sync::OnceCell::new();

pub fn get_or_init_registry_for_endpoint(endpoint: String) -> SharedRegistry {
    let map_lock =
        REGISTRY_ADDRESSES.get_or_init(|| parking_lot::RwLock::new(HashMap::new()));

    {
        let map = map_lock.read();
        if let Some(registry) = map.get(&endpoint) {
            return registry.clone();
        }
    }

    let new_registry = Arc::new(Mutex::new(ConnectionRegistry::new()));

    {
        let mut map = map_lock.write();
        map.insert(endpoint.to_string(), new_registry.clone());
    }

    new_registry
}

/// Python-exposed WebSocketRegistry that delegates to the shared registry
#[derive(Default)]
#[pyclass]
pub struct WebSocketRegistry {
    pub registry: Option<SharedRegistry>,
    pub endpoint: String,
}

#[pymethods]
impl WebSocketRegistry {
    #[new]
    pub fn new() -> Self {
        Self {
            registry: None,
            endpoint: String::new(),
        }
    }

    pub fn _init_registry(&mut self, endpoint: String) {
        self.registry = Some(get_or_init_registry_for_endpoint(endpoint.clone()));
        self.endpoint = endpoint;
    }
}
