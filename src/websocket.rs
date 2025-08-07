use crate::server::DonationItem;
use crate::server::GameHistoryItem;
use crate::server::WebSocketMessage;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct WebSocketBroadcaster {
    tx: broadcast::Sender<String>,
}

impl Default for WebSocketBroadcaster {
 fn default() -> Self {
     Self::new()
 }
}

impl WebSocketBroadcaster {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn broadcast_message(&self, message: WebSocketMessage) -> Result<(), String> {
        let json_message = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize websocket message: {e}"))?;

        // Ignore send errors (no receivers)
        let _ = self.tx.send(json_message);
        Ok(())
    }

    // Backward compatibility method
    pub fn broadcast_game_result(&self, game: GameHistoryItem) -> Result<(), String> {
        self.broadcast_message(WebSocketMessage::GameResult(game))
    }

    pub fn broadcast_donation(&self, donation: DonationItem) -> Result<(), String> {
        self.broadcast_message(WebSocketMessage::Donation(donation))
    }

    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

pub type SharedBroadcaster = Arc<RwLock<WebSocketBroadcaster>>;
