//! WebSocket relay state and broadcast.

use tokio::sync::broadcast;

use shared::{WsCommandNewPayload, WsCommandUpdatePayload};

/// Message to broadcast to WebSocket clients.
#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    CommandNew(WsCommandNewPayload),
    CommandUpdate(WsCommandUpdatePayload),
}

/// Relay state: broadcast channel for WebSocket messages.
#[derive(Clone)]
pub struct RelayState {
    tx: broadcast::Sender<BroadcastMessage>,
}

impl RelayState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BroadcastMessage> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, msg: BroadcastMessage) {
        let _ = self.tx.send(msg);
    }
}
