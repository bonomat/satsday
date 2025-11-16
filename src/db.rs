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

pub async fn register_telegram_chat(pool: &Pool<Sqlite>, chat_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT OR IGNORE INTO telegram_registrations (chat_id)
        VALUES (?)
        "#,
        chat_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn unregister_telegram_chat(
    pool: &Pool<Sqlite>,
    chat_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        DELETE FROM telegram_registrations
        WHERE chat_id = ?
        "#,
        chat_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_registered_telegram_chats(
    pool: &Pool<Sqlite>,
) -> Result<Vec<String>, sqlx::Error> {
    let records = sqlx::query!(
        r#"
        SELECT chat_id
        FROM telegram_registrations
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(records.into_iter().map(|r| r.chat_id).collect())
}

pub async fn is_telegram_chat_registered(
    pool: &Pool<Sqlite>,
    chat_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM telegram_registrations
        WHERE chat_id = ?
        "#,
        chat_id
    )
    .fetch_one(pool)
    .await?;

    Ok(result.count > 0)
}

#[derive(Debug)]
pub struct DatabaseStats {
    pub total_games: i64,
    pub total_winners: i64,
    pub total_losers: i64,
    pub unpaid_winners: i64,
    pub total_bet_amount: i64,
    pub total_payout_amount: i64,
    pub total_house_profit: i64,
}

#[derive(Debug)]
pub struct MultiplierStats {
    pub multiplier: i64,
    pub total_games: i64,
    pub total_winners: i64,
    pub total_losers: i64,
    pub total_bet_amount: i64,
    pub total_payout_amount: i64,
}

pub async fn get_database_stats(pool: &Pool<Sqlite>) -> Result<DatabaseStats, sqlx::Error> {
    let total_games = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_results
        WHERE rolled_number != -1
        "#
    )
    .fetch_one(pool)
    .await?
    .count;

    let total_winners = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_results
        WHERE is_winner = TRUE
        "#
    )
    .fetch_one(pool)
    .await?
    .count;

    let total_losers = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_results
        WHERE is_winner = FALSE AND rolled_number != -1
        "#
    )
    .fetch_one(pool)
    .await?
    .count;

    let unpaid_winners = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM game_results
        WHERE is_winner = TRUE AND payment_successful = FALSE
        "#
    )
    .fetch_one(pool)
    .await?
    .count;

    let bet_stats = sqlx::query!(
        r#"
        SELECT
            COALESCE(SUM(bet_amount), 0) as total_bet,
            COALESCE(SUM(CASE WHEN winning_amount IS NOT NULL THEN winning_amount ELSE 0 END), 0) as total_payout
        FROM game_results
        WHERE rolled_number != -1
        "#
    )
    .fetch_one(pool)
    .await?;

    let total_bet_amount = bet_stats.total_bet;
    let total_payout_amount = bet_stats.total_payout;
    let total_house_profit = total_bet_amount - total_payout_amount;

    Ok(DatabaseStats {
        total_games,
        total_winners,
        total_losers,
        unpaid_winners,
        total_bet_amount,
        total_payout_amount,
        total_house_profit,
    })
}

pub async fn get_stats_by_multiplier(
    pool: &Pool<Sqlite>,
) -> Result<Vec<MultiplierStats>, sqlx::Error> {
    let stats = sqlx::query!(
        r#"
        SELECT
            multiplier,
            COUNT(*) as total_games,
            SUM(CASE WHEN is_winner = TRUE THEN 1 ELSE 0 END) as total_winners,
            SUM(CASE WHEN is_winner = FALSE THEN 1 ELSE 0 END) as total_losers,
            SUM(bet_amount) as total_bet,
            COALESCE(SUM(CASE WHEN winning_amount IS NOT NULL THEN winning_amount ELSE 0 END), 0) as total_payout
        FROM game_results
        WHERE rolled_number != -1
        GROUP BY multiplier
        ORDER BY multiplier ASC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(stats
        .into_iter()
        .map(|s| MultiplierStats {
            multiplier: s.multiplier,
            total_games: s.total_games,
            total_winners: s.total_winners,
            total_losers: s.total_losers,
            total_bet_amount: s.total_bet,
            total_payout_amount: s.total_payout,
        })
        .collect())
}
