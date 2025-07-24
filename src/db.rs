use sqlx::{Pool, Sqlite};
use time::OffsetDateTime;

#[derive(Debug, sqlx::FromRow)]
pub struct Nonce {
    pub id: i64,
    pub nonce: String,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

pub async fn insert_nonce(
    pool: &Pool<Sqlite>,
    nonce: &str,
    expires_at: OffsetDateTime,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO nonces (nonce, expires_at)
        VALUES (?, ?)
        "#,
        nonce,
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
        SELECT id, nonce, created_at, expires_at
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
