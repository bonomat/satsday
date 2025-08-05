use crate::server::GameHistoryItem;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct WebSocketBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WebSocketBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn broadcast_game_result(&self, game: GameHistoryItem) -> Result<(), String> {
        let message = serde_json::to_string(&game)
            .map_err(|e| format!("Failed to serialize game result: {}", e))?;

        // Ignore send errors (no receivers)
        let _ = self.tx.send(message);
        Ok(())
    }

    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

pub type SharedBroadcaster = Arc<RwLock<WebSocketBroadcaster>>;
