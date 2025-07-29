use anyhow::Result;
use axum::{Router, extract::State, http::StatusCode, response::Json, routing::get};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::{
    ArkClient, nonce_service::spawn_nonce_service, transaction_processor::spawn_transaction_monitor,
};

#[derive(Clone)]
pub struct AppState {
    pub ark_client: Arc<ArkClient>,
}

#[derive(Serialize)]
struct GameAddressInfo {
    address: String,
    multiplier: String,
    multiplier_value: u64,
    max_roll: u16,
    win_probability: f64,
}

pub async fn start_server(ark_client: ArkClient, port: u16, pool: Pool<Sqlite>) -> Result<()> {
    let ark_client_arc = Arc::new(ark_client);

    // Get our addresses for transaction monitoring
    let my_addresses = vec![ark_client_arc.get_address()];

    let state = AppState {
        ark_client: ark_client_arc.clone(),
    };

    // Start nonce service (generate new nonce every 24 hours)
    let nonce_service = spawn_nonce_service(pool.clone(), 1, 1).await;

    // Start transaction monitoring in background
    spawn_transaction_monitor(ark_client_arc, my_addresses, 10, nonce_service, pool).await;
    println!("🔍 Transaction monitoring started (checking every 10 seconds)");

    let app = Router::new()
        .route("/address", get(get_address))
        .route("/boarding-address", get(get_boarding_address))
        .route("/game-addresses", get(get_game_addresses))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    println!("🚀 Server starting on http://{addr}");
    println!("📍 Address endpoint: http://{addr}/address");
    println!("🚢 Boarding address endpoint: http://{addr}/boarding-address");
    println!("🎮 Game addresses endpoint: http://{addr}/game-addresses");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn get_address(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let address = state.ark_client.get_address();

    Ok(Json(json!({
        "address": address.to_string()
    })))
}

async fn get_boarding_address(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let boarding_address = state.ark_client.get_boarding_address();

    Ok(Json(json!({
        "boarding_address": boarding_address.to_string()
    })))
}

async fn get_game_addresses(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let game_addresses = state.ark_client.get_game_addresses();

    let addresses: Vec<GameAddressInfo> = game_addresses
        .into_iter()
        .map(|(multiplier, address)| {
            let win_probability = multiplier.get_lower_than() as f64 / 65536.0 * 100.0;

            GameAddressInfo {
                address: address.encode(),
                multiplier: multiplier.to_string(),
                multiplier_value: multiplier.multiplier(),
                max_roll: multiplier.get_lower_than(),
                win_probability,
            }
        })
        .collect();

    Ok(Json(json!({
        "game_addresses": addresses,
        "info": {
            "roll_range": "0-65535",
            "win_condition": "rolled_number < max_roll"
        }
    })))
}
