use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::{transaction_processor::spawn_transaction_monitor, ArkClient};

#[derive(Clone)]
pub struct AppState {
    pub ark_client: Arc<ArkClient>,
}

pub async fn start_server(ark_client: ArkClient, port: u16) -> Result<()> {
    let ark_client_arc = Arc::new(ark_client);
    
    let state = AppState {
        ark_client: ark_client_arc.clone(),
    };

    // Start transaction monitoring in background
    spawn_transaction_monitor(ark_client_arc, 10).await; // Check every 10 seconds
    println!("🔍 Transaction monitoring started (checking every 10 seconds)");

    let app = Router::new()
        .route("/address", get(get_address))
        .route("/boarding-address", get(get_boarding_address))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    
    println!("🚀 Server starting on http://{}", addr);
    println!("📍 Address endpoint: http://{}/address", addr);
    println!("🚢 Boarding address endpoint: http://{}/boarding-address", addr);

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