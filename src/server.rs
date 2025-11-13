use crate::db::get_game_results_paginated;
use crate::db::get_total_game_count;
use crate::nonce_service::spawn_nonce_service;
use crate::transaction_processor::spawn_transaction_monitor;
use crate::websocket::SharedBroadcaster;
use crate::websocket::WebSocketBroadcaster;
use crate::ArkClient;
use crate::Config;
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
use bitcoin::Amount;
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
    pub config: Config,
}

#[derive(Serialize)]
struct GameAddressInfo {
    game_type: u8,
    address: String,
    multiplier: String,
    multiplier_value: u64,
    max_roll: u16,
    win_probability: f64,
    max_bet_amount: u64,
}

#[derive(Deserialize)]
struct PaginationQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct GameHistoryItem {
    pub id: String,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub amount_sent: Amount,
    pub multiplier: f64,
    pub result_number: i64,
    pub target_number: i64,
    pub is_win: bool,
    #[serde(with = "bitcoin::amount::serde::as_sat::opt")]
    pub payout: Option<Amount>,
    pub input_tx_id: String,
    pub output_tx_id: Option<String>,
    pub nonce: Option<String>,
    pub nonce_hash: String,
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
}

#[derive(Serialize, Clone)]
pub struct DonationItem {
    pub id: String,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub amount: Amount,
    pub sender: String,
    pub input_tx_id: String,
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    GameResult(GameHistoryItem),
    Donation(DonationItem),
}

#[derive(Serialize)]
struct GameHistoryResponse {
    games: Vec<GameHistoryItem>,
    total: i64,
    page: i64,
    page_size: i64,
    total_pages: i64,
}

#[derive(Serialize)]
struct GameStatsItem {
    game_type: String,
    multiplier: String,
    address: String,
    number_of_games: usize,
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    total_received: Amount,
}

#[derive(Serialize)]
struct StatsResponse {
    total_games: usize,
    game_stats: Vec<GameStatsItem>,
}

pub async fn start_server(
    ark_client: ArkClient,
    port: u16,
    pool: Pool<Sqlite>,
    config: Config,
) -> Result<()> {
    let ark_client_arc = Arc::new(ark_client);

    // Get our addresses for transaction monitoring
    let my_addresses = vec![ark_client_arc.get_address()];

    // Create WebSocket broadcaster
    let broadcaster = Arc::new(tokio::sync::RwLock::new(WebSocketBroadcaster::default()));

    // Start nonce service (generate new nonce every 24 hours)
    let nonce_service = spawn_nonce_service(pool.clone(), 1, 1).await;

    let state = AppState {
        ark_client: ark_client_arc.clone(),
        pool: pool.clone(),
        broadcaster: broadcaster.clone(),
        nonce_service: nonce_service.clone(),
        config: config.clone(),
    };

    let dust_amount = ark_client_arc.dust_value();

    // Start transaction monitoring in background
    spawn_transaction_monitor(
        ark_client_arc,
        my_addresses,
        nonce_service,
        pool,
        broadcaster,
        config.max_payout_sats,
        dust_amount
    )
    .await;
    tracing::info!("🔍 Transaction monitoring started with subscriptions");

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
        "https://mutinynet.satsday.xyz".parse::<HeaderValue>()?,
        "https://signet.satsday.xyz".parse::<HeaderValue>()?,
    ]);

    let app = Router::new()
        .route("/address", get(get_address))
        .route("/boarding-address", get(get_boarding_address))
        .route("/game-addresses", get(get_game_addresses))
        .route("/games", get(get_games))
        .route("/stats", get(get_stats))
        .route("/version", get(get_version))
        .route("/balance", get(get_balance))
        .route("/ws", get(websocket_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("🚀 Server starting on http://{addr}");
    tracing::info!("📍 Address endpoint: http://{addr}/address");
    tracing::info!("🚢 Boarding address endpoint: http://{addr}/boarding-address");
    tracing::info!("🎮 Game addresses endpoint: http://{addr}/game-addresses");
    tracing::info!("📊 Games history endpoint: http://{addr}/games");
    tracing::info!("📈 Stats endpoint: http://{addr}/stats");
    tracing::info!("ℹ️ Version endpoint: http://{addr}/version");
    tracing::info!("💰 Balance endpoint: http://{addr}/balance");
    tracing::info!("🔌 WebSocket endpoint: ws://{addr}/ws");

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
        .map(|(game_type, multiplier, address)| {
            let win_probability = multiplier.get_lower_than() as f64 / 65536.0 * 100.0;
            // Calculate max bet amount: max_payout * 100 / multiplier
            let max_bet_amount = (state.config.max_payout_sats * 100) / multiplier.multiplier();

            GameAddressInfo {
                game_type: game_type as u8,
                address: address.encode(),
                multiplier: multiplier.to_string(),
                multiplier_value: multiplier.multiplier(),
                max_roll: multiplier.get_lower_than(),
                win_probability,
                max_bet_amount,
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
            amount_sent: Amount::from_sat(game.bet_amount as u64),
            multiplier: game.multiplier as f64 / 100.0,
            result_number: game.rolled_number,
            target_number,
            is_win: game.is_winner,
            payout: game.winning_amount.map(|a| Amount::from_sat(a as u64)),
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

async fn get_stats(State(state): State<AppState>) -> Result<Json<StatsResponse>, StatusCode> {
    let game_addresses = state.ark_client.get_game_addresses();
    let addresses_only: Vec<_> = game_addresses
        .iter()
        .map(|(_, _, address)| *address)
        .collect();

    let vtxos = state
        .ark_client
        .list_vtxos(addresses_only.as_slice())
        .await
        .map_err(|e| {
            tracing::error!("Failed to get VTXOs for stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total_games = vtxos.len();
    let mut game_stats = Vec::new();

    for (game_type, multiplier, ark_address) in game_addresses {
        let per_address: Vec<_> = vtxos
            .iter()
            .filter(|vtxo| vtxo.script == ark_address.to_p2tr_script_pubkey())
            .collect();

        let total_received: Amount = per_address.iter().map(|v| v.amount).sum();

        game_stats.push(GameStatsItem {
            game_type: game_type.to_string(),
            multiplier: multiplier.to_string(),
            address: ark_address.encode(),
            number_of_games: per_address.len(),
            total_received,
        });
    }

    Ok(Json(StatsResponse {
        total_games,
        game_stats,
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
                    amount_sent: Amount::from_sat(game.bet_amount as u64),
                    multiplier: game.multiplier as f64 / 100.0,
                    result_number: game.rolled_number,
                    target_number,
                    is_win: game.is_winner,
                    payout: game.winning_amount.map(|a| Amount::from_sat(a as u64)),
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
    let mut receiver_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Ping(_data) => {
                    tracing::trace!(target: "websocket", "Received ping, sending pong");
                    // Note: pong is handled automatically by axum's WebSocket implementation
                }
                Message::Pong(_) => {
                    tracing::trace!(target: "websocket", "Received pong");
                }
                Message::Close(_) => {
                    tracing::trace!(target: "websocket", "WebSocket connection closed by client");
                    break;
                }
                _ => {}
            }
        }
    });

    // Create interval for sending periodic pings to keep connection alive
    let mut ping_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

    // Spawn task to send real-time updates and periodic pings
    let mut sender_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle broadcast messages
                Ok(msg) = rx.recv() => {
                    if sender.send(Message::Text(msg.into())).await.is_err() {
                        tracing::debug!("Failed to send message, client disconnected");
                        break;
                    }
                }
                // Send periodic ping to keep connection alive
                _ = ping_interval.tick() => {
                    if sender.send(Message::Ping(vec![].into())).await.is_err() {
                        tracing::debug!("Failed to send ping, client disconnected");
                        break;
                    }
                    tracing::trace!("Sent ping to keep WebSocket alive");
                }
            }
        }
    });

    // Wait for either task to complete and cleanup both
    tokio::select! {
        _ = &mut receiver_handle => {
            tracing::debug!("WebSocket receiver task completed, aborting sender");
            sender_handle.abort();
            // Only await the aborted handle
            let _ = sender_handle.await;
        }
        _ = &mut sender_handle => {
            tracing::debug!("WebSocket sender task completed, aborting receiver");
            receiver_handle.abort();
            // Only await the aborted handle
            let _ = receiver_handle.await;
        }
    }

    tracing::info!("WebSocket connection fully closed and cleaned up");
}
