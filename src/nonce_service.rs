use rand::{Rng, random};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};

#[derive(Clone)]
pub struct NonceService {
    current_nonce: Arc<RwLock<u64>>,
}

impl NonceService {
    pub fn new() -> Self {
        let initial_nonce = rand::thread_rng().r#gen::<u64>();
        Self {
            current_nonce: Arc::new(RwLock::new(initial_nonce)),
        }
    }

    pub async fn get_current_nonce(&self) -> u64 {
        *self.current_nonce.read().await
    }

    pub async fn start_periodic_generation(&self, interval_hours: u64) {
        let nonce_arc = self.current_nonce.clone();

        tokio::spawn(async move {
            let mut timer = interval(Duration::from_secs(interval_hours * 3600));
            timer.tick().await; // Skip first immediate tick

            loop {
                timer.tick().await;

                let new_nonce = random::<u64>();
                {
                    let mut nonce = nonce_arc.write().await;
                    *nonce = new_nonce;
                }

                tracing::info!("🎲 Generated new nonce: {}", new_nonce);
            }
        });
    }
}

pub async fn spawn_nonce_service(interval_hours: u64) -> NonceService {
    let service = NonceService::new();

    tracing::info!(
        "🎯 Starting nonce service (generating new nonce every {} hours)",
        interval_hours
    );
    tracing::info!("🎲 Initial nonce: {}", service.get_current_nonce().await);

    service.start_periodic_generation(interval_hours).await;

    service
}
