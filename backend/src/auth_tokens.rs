//! One-time token primitives for a future SMTP delivery adapter.
//! No unauthenticated reset/verification endpoints are exposed without that adapter.
use crate::{
    auth::{hash_token, random_token},
    error::{AppError, AppResult},
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum TokenPurpose {
    VerifyEmail,
    ResetPassword,
}
impl TokenPurpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::VerifyEmail => "verify_email",
            Self::ResetPassword => "reset_password",
        }
    }
}

/// The raw value is returned once for delivery, and must never be logged or persisted.
pub async fn issue(pool: &PgPool, user: Uuid, purpose: TokenPurpose) -> AppResult<String> {
    let raw = random_token();
    let mut tx = pool.begin().await?;
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    sqlx::query("UPDATE auth_action_tokens SET consumed_at=now() WHERE user_id=$1 AND purpose=$2 AND consumed_at IS NULL").bind(user).bind(purpose.as_str()).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO auth_action_tokens(id,user_id,purpose,token_hash,expires_at) VALUES($1,$2,$3,$4,now()+interval '15 minutes')")
        .bind(Uuid::new_v4()).bind(user).bind(purpose.as_str()).bind(hash_token(&raw)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(raw)
}

/// Consume inside the same transaction as the verified-email/password change.
/// Concurrent consumption succeeds exactly once; purpose and expiry are checked in SQL.
pub async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    raw: &str,
    purpose: TokenPurpose,
) -> AppResult<Uuid> {
    if raw.len() != 43 {
        return Err(AppError::Unauthorized);
    }
    let user: Uuid = sqlx::query_scalar(
        "SELECT user_id FROM auth_action_tokens WHERE token_hash=$1 AND purpose=$2",
    )
    .bind(hash_token(raw))
    .bind(purpose.as_str())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    // Match issue()'s lock order before touching a token, including for password updates.
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    sqlx::query_scalar("UPDATE auth_action_tokens SET consumed_at=now() WHERE token_hash=$1 AND purpose=$2 AND consumed_at IS NULL AND expires_at>now() RETURNING user_id")
        .bind(hash_token(raw)).bind(purpose.as_str()).fetch_optional(&mut **tx).await?.ok_or(AppError::Unauthorized)
}
