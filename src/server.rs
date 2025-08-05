use crate::db::get_game_results_paginated;
use crate::db::get_total_game_count;
use crate::nonce_service::spawn_nonce_service;
use crate::transaction_processor::spawn_transaction_monitor;
use crate::websocket::SharedBroadcaster;
use crate::websocket::WebSocketBroadcaster;
use crate::ArkClient;
use anyhow::Result;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::StatusCode;
use axum::response::Json;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::types::time::OffsetDateTime;
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub ark_client: Arc<ArkClient>,
    pub pool: Pool<Sqlite>,
    pub broadcaster: SharedBroadcaster,
    pub nonce_service: crate::nonce_service::NonceService,
}

#[derive(Serialize)]
struct GameAddressInfo {
    address: String,
    multiplier: String,
    multiplier_value: u64,
    max_roll: u16,
    win_probability: f64,
}

#[derive(Deserialize)]
struct PaginationQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct GameHistoryItem {
    pub id: String,
    pub amount_sent: String,
    pub multiplier: f64,
    pub result_number: i64,
    pub target_number: i64,
    pub is_win: bool,
    pub payout: String,
    pub input_tx_id: String,
    pub output_tx_id: Option<String>,
    pub nonce: Option<String>,
    pub nonce_hash: String,
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
}

#[derive(Serialize)]
struct GameHistoryResponse {
    games: Vec<GameHistoryItem>,
    total: i64,
    page: i64,
    page_size: i64,
    total_pages: i64,
}

pub async fn start_server(ark_client: ArkClient, port: u16, pool: Pool<Sqlite>) -> Result<()> {
    let ark_client_arc = Arc::new(ark_client);

    // Get our addresses for transaction monitoring
    let my_addresses = vec![ark_client_arc.get_address()];

    // Create WebSocket broadcaster
    let broadcaster = Arc::new(tokio::sync::RwLock::new(WebSocketBroadcaster::new()));

    // Start nonce service (generate new nonce every 24 hours)
    let nonce_service = spawn_nonce_service(pool.clone(), 1, 1).await;

    let state = AppState {
        ark_client: ark_client_arc.clone(),
        pool: pool.clone(),
        broadcaster: broadcaster.clone(),
        nonce_service: nonce_service.clone(),
    };

    // Start transaction monitoring in background
    spawn_transaction_monitor(
        ark_client_arc,
        my_addresses,
        10,
        nonce_service,
        pool,
        broadcaster,
    )
    .await;
    println!("🔍 Transaction monitoring started (checking every 10 seconds)");

    let cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(vec![
            axum::http::header::ORIGIN,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            axum::http::header::CONTENT_TYPE,
        ]);

    let cors = cors.allow_origin([
        "http://localhost:8080".parse::<HeaderValue>()?,
        "http://localhost:12346".parse::<HeaderValue>()?,
        "http://localhost:12347".parse::<HeaderValue>()?,
        "https://satsday.xyz".parse::<HeaderValue>()?,
        "https://signet.satsday.xyz".parse::<HeaderValue>()?,
    ]);

    let app = Router::new()
        .route("/address", get(get_address))
        .route("/boarding-address", get(get_boarding_address))
        .route("/game-addresses", get(get_game_addresses))
        .route("/games", get(get_games))
        .route("/version", get(get_version))
        .route("/balance", get(get_balance))
        .route("/ws", get(websocket_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    println!("🚀 Server starting on http://{addr}");
    println!("📍 Address endpoint: http://{addr}/address");
    println!("🚢 Boarding address endpoint: http://{addr}/boarding-address");
    println!("🎮 Game addresses endpoint: http://{addr}/game-addresses");
    println!("📊 Games history endpoint: http://{addr}/games");
    println!("ℹ️ Version endpoint: http://{addr}/version");
    println!("💰 Balance endpoint: http://{addr}/balance");
    println!("🔌 WebSocket endpoint: ws://{addr}/ws");

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

async fn get_games(
    State(state): State<AppState>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<GameHistoryResponse>, StatusCode> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);

    let games = get_game_results_paginated(&state.pool, page, page_size)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total = get_total_game_count(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    let mut game_items: Vec<GameHistoryItem> = Vec::new();

    for game in games {
        let target_number = (65536.0 * 1000.0 / game.multiplier as f64) as i64;

        let revealable_nonce = state.nonce_service.get_revealable_nonce(&game.nonce).await;
        let nonce_hash = if revealable_nonce.is_some() {
            // If we can reveal the nonce, calculate its hash for verification
            let mut hasher = Sha256::new();
            hasher.update(&game.nonce);
            format!("{:x}", hasher.finalize())
        } else {
            // If we can't reveal it, it's the current nonce, so get its hash
            state.nonce_service.get_current_nonce_hash().await
        };

        game_items.push(GameHistoryItem {
            id: game.id.to_string(),
            amount_sent: format!("{:.8} BTC", game.bet_amount as f64 / 100_000_000.0),
            multiplier: game.multiplier as f64 / 1000.0,
            result_number: game.rolled_number,
            target_number,
            is_win: game.is_winner,
            payout: if game.is_winner {
                format!(
                    "{:.8} BTC",
                    game.winning_amount.unwrap_or(0) as f64 / 100_000_000.0
                )
            } else {
                "0 BTC".to_string()
            },
            input_tx_id: game.input_tx_id,
            output_tx_id: game.output_tx_id,
            nonce: revealable_nonce,
            nonce_hash,
            timestamp: game.timestamp,
        });
    }

    Ok(Json(GameHistoryResponse {
        games: game_items,
        total,
        page,
        page_size,
        total_pages,
    }))
}

async fn get_version() -> Result<Json<Value>, StatusCode> {
    const GIT_HASH: &str = env!("GIT_HASH");
    const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

    Ok(Json(json!({
        "git_hash": GIT_HASH,
        "build_timestamp": BUILD_TIMESTAMP
    })))
}

async fn get_balance(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let balance = state
        .ark_client
        .get_balance()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "offchain": {
            "spendable": balance.offchain_spendable.to_sat(),
            "expired": balance.offchain_expired.to_sat()
        },
        "boarding": {
            "spendable": balance.boarding_spendable.to_sat(),
            "expired": balance.boarding_expired.to_sat(),
            "pending": balance.boarding_pending.to_sat()
        }
    })))
}

async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: axum::extract::ws::WebSocket, state: AppState) {
    use axum::extract::ws::Message;
    use futures_util::SinkExt;
    use futures_util::StreamExt;

    let (mut sender, mut receiver) = socket.split();

    // Send historical data first
    match get_game_results_paginated(&state.pool, 1, 20).await {
        Ok(games) => {
            let mut game_items: Vec<GameHistoryItem> = Vec::new();

            for game in games {
                let target_number = (65536.0 * 1000.0 / game.multiplier as f64) as i64;

                let revealable_nonce = state.nonce_service.get_revealable_nonce(&game.nonce).await;
                let nonce_hash = if revealable_nonce.is_some() {
                    // If we can reveal the nonce, calculate its hash for verification
                    let mut hasher = Sha256::new();
                    hasher.update(&game.nonce);
                    format!("{:x}", hasher.finalize())
                } else {
                    // If we can't reveal it, it's the current nonce, so get its hash
                    state.nonce_service.get_current_nonce_hash().await
                };

                game_items.push(GameHistoryItem {
                    id: game.id.to_string(),
                    amount_sent: format!("{:.8} BTC", game.bet_amount as f64 / 100_000_000.0),
                    multiplier: game.multiplier as f64 / 1000.0,
                    result_number: game.rolled_number,
                    target_number,
                    is_win: game.is_winner,
                    payout: if game.is_winner {
                        format!(
                            "{:.8} BTC",
                            game.winning_amount.unwrap_or(0) as f64 / 100_000_000.0
                        )
                    } else {
                        "0 BTC".to_string()
                    },
                    input_tx_id: game.input_tx_id,
                    output_tx_id: game.output_tx_id,
                    nonce: revealable_nonce,
                    nonce_hash,
                    timestamp: game.timestamp,
                });
            }

            // Send initial history
            let history_msg = json!({
                "type": "history",
                "games": game_items
            });

            if let Ok(msg_str) = serde_json::to_string(&history_msg) {
                let _ = sender.send(Message::Text(msg_str.into())).await;
            }
        }
        Err(e) => {
            tracing::error!("Failed to get game history: {}", e);
        }
    }

    // Subscribe to real-time updates
    let mut rx = {
        let broadcaster = state.broadcaster.read().await;
        broadcaster.subscribe()
    };

    // Spawn task to handle incoming messages (ping/pong)
    let receiver_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Ping(_) => {
                    tracing::debug!("Received ping, sending pong");
                }
                Message::Close(_) => {
                    tracing::debug!("WebSocket connection closed by client");
                    break;
                }
                _ => {}
            }
        }
    });

    // Spawn task to send real-time updates
    let sender_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Wait for either task to complete (connection closed or error)
    tokio::select! {
        _ = receiver_task => {
            tracing::debug!("WebSocket receiver task completed");
        }
        _ = sender_task => {
            tracing::debug!("WebSocket sender task completed");
        }
    }

    tracing::debug!("WebSocket connection closed");
}
