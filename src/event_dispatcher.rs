use crate::game_data::{Card, WinState};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct EventDispatcher {
    sender: Arc<Mutex<broadcast::Sender<GameEvent>>>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        EventDispatcher {
            sender: Arc::new(Mutex::new(sender)),
        }
    }

    pub async fn subscribe(&self) -> broadcast::Receiver<GameEvent> {
        let sender = self.sender.lock().await;
        sender.subscribe()
    }

    pub async fn dispatch(&self, event: GameEvent) {
        let sender = self.sender.lock().await;
        if sender.send(event).is_err() {
            eprintln!("Failed to dispatch event!");
        }
    }
}
#[derive(Debug, Clone)]
pub enum GameEvent {
    PlayerHit(Card),
    PlayerStand,
    PlayerSplit(Card),
    PlayerInsure,
    DealerHit(Card),
    DealerStand,
    RoundResult(WinState),
    Error(String),
}