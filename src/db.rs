use sqlx::Pool;
use sqlx::Sqlite;
use time::OffsetDateTime;

#[derive(Debug, sqlx::FromRow)]
pub struct Nonce {
    pub id: i64,
    pub nonce: String,
    pub nonce_hash: String,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, sqlx::FromRow)]
pub struct GameResult {
    pub id: i64,
    pub nonce: String,
    pub rolled_number: i64,
    pub input_tx_id: String,
    pub output_tx_id: Option<String>,
    pub bet_amount: i64,
    pub winning_amount: Option<i64>,
    pub player_address: String,
    pub is_winner: bool,
    pub payment_successful: bool,
    pub timestamp: OffsetDateTime,
    pub multiplier: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct OwnTransaction {
    pub id: i64,
    pub tx_id: String,
    pub transaction_type: String,
    pub created_at: OffsetDateTime,
}

pub async fn insert_nonce(
    pool: &Pool<Sqlite>,
    nonce: &str,
    nonce_hash: &str,
    expires_at: OffsetDateTime,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO nonces (nonce, nonce_hash, expires_at)
        VALUES (?, ?, ?)
        "#,
        nonce,
        nonce_hash,
        expires_at
    )
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn get_nonce(pool: &Pool<Sqlite>, nonce: &str) -> Result<Option<Nonce>, sqlx::Error> {
    let nonce = sqlx::query_as!(
        Nonce,
        r#"
        SELECT id, nonce, nonce_hash, created_at, expires_at
        FROM nonces
        WHERE nonce = ?
        "#,
        nonce
    )
    .fetch_optional(pool)
    .await?;

    Ok(nonce)
}

pub async fn is_nonce_valid(pool: &Pool<Sqlite>, nonce: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM nonces
        WHERE nonce = ? AND expires_at > datetime('now')
        "#,
        nonce
    )
    .fetch_one(pool)
    .await?;

    Ok(result.count > 0)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_game_result(
    pool: &Pool<Sqlite>,
    nonce: &str,
    rolled_number: i64,
    input_tx_id: &str,
    output_tx_id: Option<&str>,
    bet_amount: i64,
    winning_amount: Option<i64>,
    player_address: &str,
    is_winner: bool,
    payment_successful: bool,
    multiplier: i64,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO game_results (
            nonce, rolled_number, input_tx_id, output_tx_id,
            bet_amount, winning_amount, player_address,
            is_winner, payment_successful, multiplier
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        nonce,
        rolled_number,
        input_tx_id,
        output_tx_id,
        bet_amount,
        winning_amount,
        player_address,
        is_winner,
        payment_successful,
        multiplier
    )
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn is_transaction_processed(
    pool: &Pool<Sqlite>,
    input_tx_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_results
        WHERE input_tx_id = ?
        "#,
        input_tx_id
    )
    .fetch_one(pool)
    .await?;

    Ok(result.count > 0)
}

pub async fn insert_own_transaction(
    pool: &Pool<Sqlite>,
    tx_id: &str,
    transaction_type: &str,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO own_transactions (tx_id, transaction_type)
        VALUES (?, ?)
        "#,
        tx_id,
        transaction_type
    )
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn is_own_transaction(pool: &Pool<Sqlite>, tx_id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM own_transactions
        WHERE tx_id = ?
        "#,
        tx_id
    )
    .fetch_one(pool)
    .await?;

    Ok(result.count > 0)
}

pub async fn get_game_results_paginated(
    pool: &Pool<Sqlite>,
    page: i64,
    page_size: i64,
) -> Result<Vec<GameResult>, sqlx::Error> {
    let offset = (page - 1) * page_size;

    let results = sqlx::query_as!(
        GameResult,
        r#"
        SELECT id, nonce, rolled_number, input_tx_id, output_tx_id,
               bet_amount, winning_amount, player_address, is_winner,
               payment_successful, timestamp, multiplier
        FROM game_results
        ORDER BY timestamp DESC
        LIMIT ? OFFSET ?
        "#,
        page_size,
        offset
    )
    .fetch_all(pool)
    .await?;

    Ok(results)
}

pub async fn get_total_game_count(pool: &Pool<Sqlite>) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_results
        "#
    )
    .fetch_one(pool)
    .await?;

    Ok(result.count)
}

pub async fn get_unpaid_winners(pool: &Pool<Sqlite>) -> Result<Vec<GameResult>, sqlx::Error> {
    let results = sqlx::query_as!(
        GameResult,
        r#"
        SELECT id, nonce, rolled_number, input_tx_id, output_tx_id,
               bet_amount, winning_amount, player_address, is_winner,
               payment_successful, timestamp, multiplier
        FROM game_results
        WHERE is_winner = TRUE AND payment_successful = FALSE
        ORDER BY timestamp ASC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(results)
}

pub async fn mark_payment_successful(
    pool: &Pool<Sqlite>,
    game_id: i64,
    output_tx_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE game_results
        SET payment_successful = TRUE, output_tx_id = ?
        WHERE id = ?
        "#,
        output_tx_id,
        game_id
    )
    .execute(pool)
    .await?;

    Ok(())
}
