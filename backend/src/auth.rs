//! Short-lived JWT access tokens are bound to revocable, rotating refresh sessions.
use crate::{
    AppState,
    error::{AppError, AppJson, AppResult},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spendwise_shared::{
    AuthResponse, CsrfResponse, LoginInput, PasswordInput, ProfileInput, RegisterInput, User,
};
use sqlx::{FromRow, Postgres, Transaction};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration as StdDuration, Instant},
};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use validator::Validate;

const REFRESH_COOKIE: &str = "spendwise_refresh";
fn refresh_cookie_name(state: &AppState) -> &'static str {
    if state.config.cookie_secure {
        "__Host-spendwise_refresh"
    } else {
        REFRESH_COOKIE
    }
}
const USER_COLUMNS: &str = "id,email,display_name,currency,created_at";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    sid: Uuid,
    ver: i64,
    exp: usize,
    iat: usize,
    iss: String,
    aud: String,
}

pub struct AuthUser(pub Uuid);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let bearer = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["spendwise"]);
        validation.set_audience(&["spendwise-web"]);
        validation.leeway = 0;
        let claims = decode::<Claims>(
            bearer,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| AppError::Unauthorized)?
        .claims;
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM refresh_sessions s JOIN users u ON u.id=s.user_id WHERE s.id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL AND s.expires_at>now() AND u.token_version=$3)")
            .bind(claims.sid).bind(claims.sub).bind(claims.ver).fetch_one(&state.pool).await?;
        if !valid {
            return Err(AppError::Unauthorized);
        }
        Ok(Self(claims.sub))
    }
}

#[derive(Default)]
pub struct RateLimiter {
    entries: Mutex<HashMap<(IpAddr, String), (Instant, u32)>>,
}
impl RateLimiter {
    pub fn check(&self, ip: IpAddr, route: &str, limit: u32) -> AppResult<()> {
        let mut entries = self.entries.lock().map_err(|_| AppError::Internal)?;
        entries.retain(|_, (start, _)| start.elapsed() < StdDuration::from_secs(60));
        if entries.len() >= 10_000 && !entries.contains_key(&(ip, route.to_owned())) {
            return Err(AppError::RateLimited);
        }
        let entry = entries
            .entry((ip, route.to_owned()))
            .or_insert((Instant::now(), 0));
        if entry.1 >= limit {
            return Err(AppError::RateLimited);
        }
        entry.1 += 1;
        Ok(())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/csrf", get(csrf))
        .route("/auth/logout", post(logout))
        .route("/auth/logout-all", post(logout_all))
        .route("/auth/me", get(me))
        .route("/users/me", get(me).patch(update_profile))
        .route("/users/password", post(change_password))
}

#[derive(Validate)]
struct EmailValidation<'a> {
    #[validate(email, length(max = 254))]
    email: &'a str,
}

fn valid_name(name: &str) -> AppResult<()> {
    if !(1..=80).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Имя должно содержать от 1 до 80 символов".into(),
        ));
    }
    Ok(())
}
fn valid_password(password: &str) -> AppResult<()> {
    if !(12..=128).contains(&password.chars().count()) || password.len() > 512 {
        return Err(AppError::Validation(
            "Пароль должен содержать от 12 до 128 символов".into(),
        ));
    }
    Ok(())
}
fn normalize_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_lowercase();
    EmailValidation { email: &email }
        .validate()
        .map_err(|_| AppError::Validation("Укажите корректный email".into()))?;
    Ok(email)
}

pub async fn hash_password(password: String) -> AppResult<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| AppError::Internal)
    })
    .await
    .map_err(|_| AppError::Internal)?
}
async fn verify_password(password: String, hash: String) -> AppResult<bool> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(|_| AppError::Internal)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| AppError::Internal)?
}
pub(crate) fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub(crate) fn hash_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}
fn csrf_token(state: &AppState, refresh: &str) -> AppResult<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.jwt_secret.as_bytes())
        .map_err(|_| AppError::Internal)?;
    mac.update(b"spendwise-csrf-v1:");
    mac.update(refresh.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}
fn refresh_cookie(state: &AppState, headers: &HeaderMap) -> AppResult<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == refresh_cookie_name(state)
                && value.len() == 43
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
            .then(|| value.to_owned())
        })
        .ok_or(AppError::Unauthorized)
}
fn csrf_check(state: &AppState, headers: &HeaderMap, refresh: &str) -> AppResult<()> {
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Forbidden)?;
    let expected = csrf_token(state, refresh)?;
    if supplied.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1 {
        return Err(AppError::Forbidden);
    }
    Ok(())
}
fn cookie_header(state: &AppState, value: &str, clear: bool) -> AppResult<HeaderValue> {
    let max_age = if clear {
        0
    } else {
        state.config.refresh_token_days * 86_400
    };
    let name = refresh_cookie_name(state);
    let path = if state.config.cookie_secure {
        "/"
    } else {
        "/api/v1/auth"
    };
    HeaderValue::from_str(&format!(
        "{name}={value}; Path={path}; HttpOnly; SameSite=Strict; Max-Age={max_age}{}",
        if state.config.cookie_secure {
            "; Secure"
        } else {
            ""
        }
    ))
    .map_err(|_| AppError::Internal)
}
fn clear_response(state: &AppState) -> AppResult<Response> {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie_header(state, "", true)?);
    Ok(response)
}

async fn issue_session(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    user: User,
    version: i64,
    family: Option<Uuid>,
    status: StatusCode,
) -> AppResult<Response> {
    let refresh = random_token();
    let csrf = csrf_token(state, &refresh)?;
    let sid = Uuid::new_v4();
    sqlx::query("INSERT INTO refresh_sessions(id,user_id,token_hash,csrf_hash,expires_at,family_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(sid).bind(user.id).bind(hash_token(&refresh)).bind(hash_token(&csrf)).bind(Utc::now() + Duration::days(state.config.refresh_token_days)).bind(family.unwrap_or(sid)).execute(&mut **tx).await?;
    // Keep a bounded number of live devices without erasing replay evidence.
    sqlx::query("UPDATE refresh_sessions SET revoked_at=now() WHERE id IN (SELECT id FROM refresh_sessions WHERE user_id=$1 AND revoked_at IS NULL ORDER BY created_at DESC,id DESC OFFSET 20)").bind(user.id).execute(&mut **tx).await?;
    let now = Utc::now();
    let claims = Claims {
        sub: user.id,
        sid,
        ver: version,
        exp: (now + Duration::minutes(state.config.access_token_minutes)).timestamp() as usize,
        iat: now.timestamp() as usize,
        iss: "spendwise".into(),
        aud: "spendwise-web".into(),
    };
    let access_token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal)?;
    let mut response = (
        status,
        Json(AuthResponse {
            access_token,
            user,
            csrf_token: csrf,
        }),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie_header(state, &refresh, false)?);
    Ok(response)
}

async fn register(
    State(state): State<AppState>,
    AppJson(input): AppJson<RegisterInput>,
) -> AppResult<Response> {
    let email = normalize_email(&input.email)?;
    let name = input.display_name.trim();
    valid_name(name)?;
    valid_password(&input.password)?;
    let _permit = state
        .auth_work
        .acquire()
        .await
        .map_err(|_| AppError::Internal)?;
    let hash = hash_password(input.password).await?;
    let mut tx = state.pool.begin().await?;
    let user = sqlx::query_as::<_, User>(&format!("INSERT INTO users(id,email,password_hash,display_name) VALUES($1,$2,$3,$4) RETURNING {USER_COLUMNS}"))
        .bind(Uuid::new_v4()).bind(email).bind(hash).bind(name).fetch_one(&mut *tx).await?;
    seed_categories(&mut tx, user.id).await?;
    let response = issue_session(&state, &mut tx, user, 0, None, StatusCode::CREATED).await?;
    tx.commit().await?;
    Ok(response)
}

async fn seed_categories(tx: &mut Transaction<'_, Postgres>, user_id: Uuid) -> AppResult<()> {
    let categories = [
        ("Еда", "expense", "#10b981", "food"),
        ("Транспорт", "expense", "#3b82f6", "transport"),
        ("Покупки", "expense", "#8b5cf6", "shopping"),
        ("Развлечения", "expense", "#f59e0b", "entertainment"),
        ("Здоровье", "expense", "#ef4444", "health"),
        ("Образование", "expense", "#06b6d4", "education"),
        ("Счета", "expense", "#64748b", "bills"),
        ("Другое", "expense", "#94a3b8", "other"),
        ("Зарплата", "income", "#10b981", "salary"),
        ("Фриланс", "income", "#3b82f6", "freelance"),
        ("Инвестиции", "income", "#8b5cf6", "investment"),
        ("Подарки", "income", "#f59e0b", "gift"),
        ("Другое", "income", "#94a3b8", "other"),
    ];
    for (name, kind, color, icon) in categories {
        sqlx::query("INSERT INTO categories(id,user_id,name,transaction_type,color,icon) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(user_id,name,transaction_type) DO NOTHING")
            .bind(Uuid::new_v4()).bind(user_id).bind(name).bind(kind).bind(color).bind(icon).execute(&mut **tx).await?;
    }
    Ok(())
}

#[derive(FromRow)]
struct Credential {
    id: Uuid,
    password_hash: String,
}
async fn login(
    State(state): State<AppState>,
    AppJson(input): AppJson<LoginInput>,
) -> AppResult<Response> {
    let email = normalize_email(&input.email)?;
    if input.password.len() > 512 {
        return Err(AppError::Unauthorized);
    }
    let _permit = state
        .auth_work
        .acquire()
        .await
        .map_err(|_| AppError::Internal)?;
    let credential =
        sqlx::query_as::<_, Credential>("SELECT id,password_hash FROM users WHERE email=$1")
            .bind(email)
            .fetch_optional(&state.pool)
            .await?;
    let hash = credential
        .as_ref()
        .map(|c| c.password_hash.clone())
        .unwrap_or_else(|| state.dummy_password_hash.as_ref().clone());
    if !verify_password(input.password, hash.clone()).await? {
        return Err(AppError::Unauthorized);
    }
    let credential = credential.ok_or(AppError::Unauthorized)?;
    let mut tx = state.pool.begin().await?;
    let (version, current_hash): (i64, String) =
        sqlx::query_as("SELECT token_version,password_hash FROM users WHERE id=$1 FOR UPDATE")
            .bind(credential.id)
            .fetch_one(&mut *tx)
            .await?;
    if current_hash != hash {
        return Err(AppError::Unauthorized);
    }
    let user = sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id=$1"))
        .bind(credential.id)
        .fetch_one(&mut *tx)
        .await?;
    let response = issue_session(&state, &mut tx, user, version, None, StatusCode::OK).await?;
    tx.commit().await?;
    Ok(response)
}

#[derive(FromRow)]
struct Session {
    id: Uuid,
    family_id: Uuid,
    csrf_hash: String,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

async fn csrf(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<CsrfResponse>> {
    let refresh = refresh_cookie(&state, &headers)?;
    let valid:bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM refresh_sessions WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>now())").bind(hash_token(&refresh)).fetch_one(&state.pool).await?;
    if !valid {
        return Err(AppError::Unauthorized);
    }
    Ok(Json(CsrfResponse {
        csrf_token: csrf_token(&state, &refresh)?,
    }))
}

async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let raw = refresh_cookie(&state, &headers)?;
    csrf_check(&state, &headers, &raw)?;
    let mut tx = state.pool.begin().await?;
    let user_id: Uuid =
        sqlx::query_scalar("SELECT user_id FROM refresh_sessions WHERE token_hash=$1")
            .bind(hash_token(&raw))
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::Unauthorized)?;
    let version: i64 = sqlx::query_scalar("SELECT token_version FROM users WHERE id=$1 FOR UPDATE")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    let session = sqlx::query_as::<_,Session>("SELECT id,family_id,csrf_hash,expires_at,revoked_at FROM refresh_sessions WHERE token_hash=$1 FOR UPDATE").bind(hash_token(&raw)).fetch_one(&mut *tx).await?;
    if session.csrf_hash != hash_token(&csrf_token(&state, &raw)?) {
        return Err(AppError::Forbidden);
    }
    if session.revoked_at.is_some() {
        // A consumed refresh token was replayed. Revoke the whole family of sessions.
        revoke_family(&mut tx, user_id, session.family_id).await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized);
    }
    if session.expires_at <= Utc::now() {
        return Err(AppError::Unauthorized);
    }
    sqlx::query("UPDATE refresh_sessions SET revoked_at=now() WHERE id=$1")
        .bind(session.id)
        .execute(&mut *tx)
        .await?;
    let user = sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id=$1"))
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    let response = issue_session(
        &state,
        &mut tx,
        user,
        version,
        Some(session.family_id),
        StatusCode::OK,
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    let raw = refresh_cookie(&state, &headers)?;
    csrf_check(&state, &headers, &raw)?;
    let mut tx = state.pool.begin().await?;
    let owner: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT user_id,family_id FROM refresh_sessions WHERE token_hash=$1")
            .bind(hash_token(&raw))
            .fetch_optional(&mut *tx)
            .await?;
    if let Some((user, family)) = owner {
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .execute(&mut *tx)
            .await?;
        revoke_family(&mut tx, user, family).await?;
    }
    tx.commit().await?;
    clear_response(&state)
}
async fn revoke_family(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    family: Uuid,
) -> AppResult<()> {
    sqlx::query("UPDATE refresh_sessions SET revoked_at=COALESCE(revoked_at,now()) WHERE user_id=$1 AND family_id=$2").bind(user).bind(family).execute(&mut **tx).await?;
    Ok(())
}
async fn revoke_all(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE users SET token_version=token_version+1 WHERE id=$1")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE refresh_sessions SET revoked_at=COALESCE(revoked_at,now()) WHERE user_id=$1",
    )
    .bind(user)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
async fn logout_all(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    headers: HeaderMap,
) -> AppResult<Response> {
    let raw = refresh_cookie(&state, &headers)?;
    csrf_check(&state, &headers, &raw)?;
    let owner:Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM refresh_sessions WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>now()").bind(hash_token(&raw)).fetch_optional(&state.pool).await?;
    if owner != Some(user) {
        return Err(AppError::Forbidden);
    }
    let mut tx = state.pool.begin().await?;
    revoke_all(&mut tx, user).await?;
    tx.commit().await?;
    clear_response(&state)
}
async fn me(State(state): State<AppState>, AuthUser(user): AuthUser) -> AppResult<Json<User>> {
    Ok(Json(
        sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id=$1"))
            .bind(user)
            .fetch_one(&state.pool)
            .await?,
    ))
}
async fn update_profile(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AppJson(input): AppJson<ProfileInput>,
) -> AppResult<Json<User>> {
    valid_name(input.display_name.trim())?;
    if !["KZT", "USD", "EUR"].contains(&input.currency.as_str()) {
        return Err(AppError::Validation("Валюта: KZT, USD или EUR".into()));
    }
    let mut tx = state.pool.begin().await?;
    let current: String = sqlx::query_scalar("SELECT currency FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    if current != input.currency {
        let has_finances:bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE user_id=$1) OR EXISTS(SELECT 1 FROM budgets WHERE user_id=$1) OR EXISTS(SELECT 1 FROM recurring_transactions WHERE user_id=$1)").bind(user).fetch_one(&mut *tx).await?;
        if has_finances {
            return Err(AppError::Conflict("Валюту можно изменить только до создания финансовых записей; исторические суммы не конвертируются".into()));
        }
    }
    let updated = sqlx::query_as::<_,User>(&format!("UPDATE users SET display_name=$2,currency=$3,updated_at=now() WHERE id=$1 RETURNING {USER_COLUMNS}"))
        .bind(user).bind(input.display_name.trim()).bind(input.currency).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(updated))
}
async fn change_password(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    AppJson(input): AppJson<PasswordInput>,
) -> AppResult<Response> {
    valid_password(&input.new_password)?;
    if input.current_password.len() > 512 {
        return Err(AppError::Unauthorized);
    }
    let _permit = state
        .auth_work
        .acquire()
        .await
        .map_err(|_| AppError::Internal)?;
    let current: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(user)
        .fetch_one(&state.pool)
        .await?;
    if !verify_password(input.current_password, current.clone()).await? {
        return Err(AppError::Validation("Текущий пароль неверен".into()));
    }
    let next = hash_password(input.new_password).await?;
    let mut tx = state.pool.begin().await?;
    let locked: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
    if locked != current {
        return Err(AppError::Conflict(
            "Пароль уже изменён. Войдите заново".into(),
        ));
    }
    sqlx::query("UPDATE users SET password_hash=$2,updated_at=now() WHERE id=$1")
        .bind(user)
        .bind(next)
        .execute(&mut *tx)
        .await?;
    revoke_all(&mut tx, user).await?;
    tx.commit().await?;
    clear_response(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn argon_password_verification() {
        let hash = hash_password("correct long password".into()).await.unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(
            verify_password("correct long password".into(), hash.clone())
                .await
                .unwrap()
        );
        assert!(!verify_password("wrong".into(), hash).await.unwrap());
    }
    #[test]
    fn password_and_email_validation() {
        assert!(valid_password("short").is_err());
        assert!(valid_password(&"x".repeat(129)).is_err());
        assert!(normalize_email("invalid").is_err());
        assert_eq!(
            normalize_email(" Person@Example.com ").unwrap(),
            "person@example.com"
        );
    }
    #[test]
    fn limiter_enforces_bounded_window() {
        let limiter = RateLimiter::default();
        let ip = "127.0.0.1".parse().unwrap();
        assert!(limiter.check(ip, "login", 1).is_ok());
        assert!(matches!(
            limiter.check(ip, "login", 1),
            Err(AppError::RateLimited)
        ));
        assert!(limiter.check(ip, "register", 1).is_ok());
    }
}
