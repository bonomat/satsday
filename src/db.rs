use sqlx::{Pool, Sqlite};
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
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO game_results (
            nonce, rolled_number, input_tx_id, output_tx_id,
            bet_amount, winning_amount, player_address,
            is_winner, payment_successful
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        nonce,
        rolled_number,
        input_tx_id,
        output_tx_id,
        bet_amount,
        winning_amount,
        player_address,
        is_winner,
        payment_successful
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
