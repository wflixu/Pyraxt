pub mod registry;

use crate::executors::web_socket_executors::execute_ws_function;
use crate::types::function_info::FunctionInfo;
use crate::types::multimap::QueryParams;
use registry::{Close, SendMessageToAll, SendText, SharedRegistry};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use log::debug;
use pyo3::prelude::*;
use pyo3_asyncio::TaskLocals;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

/// WebSocket connector - Python-exposed wrapper for sending messages
#[derive(Clone)]
#[pyclass]
pub struct WebSocketConnector {
    pub id: Uuid,
    pub router: HashMap<String, FunctionInfo>,
    pub task_locals: TaskLocals,
    pub registry: SharedRegistry,
    pub query_params: QueryParams,
    pub sender: Option<mpsc::Sender<String>>,
}

#[pymethods]
impl WebSocketConnector {
    pub fn sync_send_to(&self, recipient_id: String, message: String) {
        let recipient_id = Uuid::parse_str(&recipient_id).unwrap();

        let registry = self.registry.clone();
        let sender_id = self.id;
        tokio::spawn(async move {
            registry
                .lock()
                .await
                .send_text(SendText {
                    message: message.to_string(),
                    sender_id,
                    recipient_id,
                })
                .await;
        });
    }

    pub fn async_send_to(
        &self,
        py: Python,
        recipient_id: String,
        message: String,
    ) -> PyResult<Py<PyAny>> {
        let registry = self.registry.clone();
        let recipient_id = Uuid::parse_str(&recipient_id).unwrap();
        let sender_id = self.id;

        let awaitable = pyo3_asyncio::tokio::future_into_py(py, async move {
            registry
                .lock()
                .await
                .send_text(SendText {
                    message: message.to_string(),
                    sender_id,
                    recipient_id,
                })
                .await;
            Ok(())
        })?;

        Ok(awaitable.into_py(py))
    }

    pub fn sync_broadcast(&self, message: String) {
        let registry = self.registry.clone();
        let sender_id = self.id;
        tokio::spawn(async move {
            registry
                .lock()
                .await
                .broadcast(SendMessageToAll {
                    message: message.to_string(),
                    sender_id,
                })
                .await;
        });
    }

    pub fn async_broadcast(&self, py: Python, message: String) -> PyResult<Py<PyAny>> {
        let registry = self.registry.clone();
        let sender_id = self.id;

        let awaitable = pyo3_asyncio::tokio::future_into_py(py, async move {
            registry
                .lock()
                .await
                .broadcast(SendMessageToAll {
                    message: message.to_string(),
                    sender_id,
                })
                .await;
            Ok(())
        })?;

        Ok(awaitable.into_py(py))
    }

    pub fn close(&self) {
        let registry = self.registry.clone();
        let id = self.id;
        tokio::spawn(async move {
            registry.lock().await.close_connection(Close { id }).await;
        });
    }

    #[getter]
    pub fn get_id(&self) -> String {
        self.id.to_string()
    }

    #[getter]
    pub fn get_query_params(&self) -> QueryParams {
        self.query_params.clone()
    }
}

/// Axum WebSocket upgrade handler
pub async fn start_web_socket(
    ws: WebSocketUpgrade,
    router: HashMap<String, FunctionInfo>,
    task_locals: TaskLocals,
    endpoint: String,
    query_string: Option<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        handle_socket(socket, router, task_locals, endpoint, query_string)
    })
}

/// Handle an upgraded WebSocket connection using the Stream model
async fn handle_socket(
    socket: WebSocket,
    router: HashMap<String, FunctionInfo>,
    task_locals: TaskLocals,
    endpoint: String,
    query_string: Option<String>,
) {
    let id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel::<String>(32);

    let registry = registry::get_or_init_registry_for_endpoint(endpoint);

    // Register this connection
    registry.lock().await.register(id, tx.clone());

    // Parse query params
    let mut query_params = QueryParams::new();
    if let Some(query) = &query_string {
        if !query.is_empty() {
            for s in query.split('&') {
                let (key, value) = s.split_once('=').unwrap_or((s, ""));
                query_params.set(key.to_string(), value.to_string());
            }
        }
    }

    // Create connector for Python callbacks
    let connector = WebSocketConnector {
        id,
        router: router.clone(),
        task_locals: task_locals.clone(),
        registry: registry.clone(),
        query_params: query_params.clone(),
        sender: Some(tx.clone()),
    };

    // Execute connect handler
    if let Some(function) = router.get("connect") {
        execute_ws_function(function, None, &task_locals, &connector);
    }

    debug!("WebSocket connected: {}", id);

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Send task: forward messages from channel to WebSocket
    let send_registry = registry.clone();
    let send_id = id;
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if msg == "Connection closed" {
                let _ = ws_sender.send(Message::Close(None)).await;
                break;
            }
            if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
        // Clean up on disconnect
        send_registry.lock().await.unregister(send_id);
    });

    // Receive task: forward messages from WebSocket to Python handler
    let recv_connector = connector.clone();
    let recv_router = router.clone();
    let recv_task_locals = task_locals.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Ping(data) => {
                    debug!("Ping message {:?}", data);
                    // Ping/Pong is handled automatically by Axum
                }
                Message::Pong(data) => {
                    debug!("Pong message {:?}", data);
                }
                Message::Text(text) => {
                    debug!("Text message received {:?}", text);
                    if let Some(function) = recv_router.get("message") {
                        execute_ws_function(
                            function,
                            Some(text.to_string()),
                            &recv_task_locals,
                            &recv_connector,
                        );
                    }
                }
                Message::Binary(bin) => {
                    debug!("Binary message received {:?}", bin.len());
                }
                Message::Close(close_frame) => {
                    debug!("Socket closing: {:?}", close_frame);
                    break;
                }
            }
        }

        // Execute close handler
        if let Some(function) = recv_router.get("close") {
            execute_ws_function(function, None, &recv_task_locals, &recv_connector);
        }

        debug!("WebSocket disconnected");
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {
            debug!("Send task completed for {}", id);
        }
        _ = recv_task => {
            debug!("Receive task completed for {}", id);
        }
    }

    // Final cleanup
    registry.lock().await.unregister(id);
}
