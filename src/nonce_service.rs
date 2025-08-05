use crate::db;
use rand::random;
use rand::Rng;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;
use time::Duration as TimeDuration;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tokio::time::interval;
use tokio::time::Duration;

#[derive(Clone)]
pub struct NonceService {
    current_nonce: Arc<RwLock<u64>>,
    db_pool: Pool<Sqlite>,
    validity_hours: u64,
}

impl NonceService {
    pub fn new(db_pool: Pool<Sqlite>, validity_hours: u64) -> Self {
        let initial_nonce = rand::thread_rng().r#gen::<u64>();
        Self {
            current_nonce: Arc::new(RwLock::new(initial_nonce)),
            db_pool,
            validity_hours,
        }
    }

    pub async fn get_current_nonce(&self) -> u64 {
        *self.current_nonce.read().await
    }

    pub async fn get_current_nonce_hash(&self) -> String {
        let nonce = self.get_current_nonce().await;
        let mut hasher = Sha256::new();
        hasher.update(nonce.to_string());
        format!("{:x}", hasher.finalize())
    }

    pub async fn verify_nonce(&self, nonce: &str) -> Result<bool, sqlx::Error> {
        db::is_nonce_valid(&self.db_pool, nonce).await
    }

    // Returns the actual nonce if it's safe to reveal (not the current one), otherwise returns None
    pub async fn get_revealable_nonce(&self, nonce_str: &str) -> Option<String> {
        let current_nonce = self.get_current_nonce().await;
        let nonce_u64 = nonce_str.parse::<u64>().ok()?;

        // Only reveal if it's not the current nonce
        if nonce_u64 != current_nonce {
            Some(nonce_str.to_string())
        } else {
            None
        }
    }

    pub async fn start_periodic_generation(&self, interval_hours: u64) {
        let nonce_arc = self.current_nonce.clone();
        let db_pool = self.db_pool.clone();
        let validity_hours = self.validity_hours;

        tokio::spawn(async move {
            let mut timer = interval(Duration::from_secs(interval_hours * 3600));
            timer.tick().await; // Skip first immediate tick

            loop {
                timer.tick().await;

                let new_nonce = random::<u64>();
                let nonce_str = new_nonce.to_string();

                // Calculate hash
                let mut hasher = Sha256::new();
                hasher.update(&nonce_str);
                let nonce_hash = format!("{:x}", hasher.finalize());

                // Store in database
                let expires_at =
                    OffsetDateTime::now_utc() + TimeDuration::hours(validity_hours as i64);
                match db::insert_nonce(&db_pool, &nonce_str, &nonce_hash, expires_at).await {
                    Ok(_) => {
                        tracing::info!(
                            "🎲 Generated new nonce: {} (expires at {})",
                            new_nonce,
                            expires_at
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to store nonce in database: {}", e);
                    }
                }

                {
                    let mut nonce = nonce_arc.write().await;
                    *nonce = new_nonce;
                }
            }
        });
    }
}

pub async fn spawn_nonce_service(
    db_pool: Pool<Sqlite>,
    interval_hours: u64,
    validity_hours: u64,
) -> NonceService {
    let service = NonceService::new(db_pool, validity_hours);

    tracing::info!(
        "🎯 Starting nonce service (generating new nonce every {} hours, valid for {} hours)",
        interval_hours,
        validity_hours
    );

    let initial_nonce = service.get_current_nonce().await;
    let nonce_str = initial_nonce.to_string();

    // Calculate hash
    let mut hasher = Sha256::new();
    hasher.update(&nonce_str);
    let nonce_hash = format!("{:x}", hasher.finalize());

    // Store initial nonce in database
    let expires_at = OffsetDateTime::now_utc() + TimeDuration::hours(validity_hours as i64);
    match db::insert_nonce(&service.db_pool, &nonce_str, &nonce_hash, expires_at).await {
        Ok(_) => {
            tracing::info!(
                "🎲 Initial nonce: {} (expires at {})",
                initial_nonce,
                expires_at
            );
        }
        Err(e) => {
            tracing::error!("Failed to store initial nonce in database: {}", e);
        }
    }

    service.start_periodic_generation(interval_hours).await;

    service
}
