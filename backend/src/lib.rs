//! SpendWise modular monolith: authenticated REST API and same-origin CSR assets.
pub mod auth;
pub mod auth_tokens;
pub mod config;
pub mod error;
pub mod finance;
pub mod repository;
pub mod services;

use crate::{
    auth::RateLimiter,
    config::Config,
    error::{AppError, AppResult},
};
use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use base64::Engine;
use serde_json::json;
use sha2::Digest;
use sqlx::PgPool;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub rate_limiter: Arc<RateLimiter>,
    pub auth_work: Arc<tokio::sync::Semaphore>,
    pub dummy_password_hash: Arc<String>,
    pub content_security_policy: HeaderValue,
}
impl AppState {
    pub async fn new(pool: PgPool, config: Config) -> AppResult<Self> {
        let content_security_policy = content_security_policy(&config.frontend_dir)?;
        Ok(Self {
            pool,
            config: Arc::new(config),
            rate_limiter: Arc::new(RateLimiter::default()),
            auth_work: Arc::new(tokio::sync::Semaphore::new(4)),
            dummy_password_hash: Arc::new(
                auth::hash_password("dummy-password-never-used-for-login".into()).await?,
            ),
            content_security_policy,
        })
    }
}

/// Authorize only the exact inline bootstrap emitted by the installed Trunk bundle.
fn content_security_policy(directory: &std::path::Path) -> AppResult<HeaderValue> {
    let mut script_sources = String::from("'self' 'wasm-unsafe-eval'");
    if let Ok(html) = std::fs::read_to_string(directory.join("index.html")) {
        for script in html.split("<script").skip(1).take(32) {
            if let Some((attributes, body)) = script.split_once('>')
                && !attributes.contains("src=")
                && let Some((source, _)) = body.split_once("</script>")
            {
                let hash = base64::engine::general_purpose::STANDARD
                    .encode(sha2::Sha256::digest(source.as_bytes()));
                script_sources.push_str(&format!(" 'sha256-{hash}'"));
            }
        }
    }
    HeaderValue::from_str(&format!("default-src 'self'; script-src {script_sources}; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'; form-action 'self'")).map_err(|_| AppError::Internal)
}

async fn not_found() -> AppError {
    AppError::NotFound
}
async fn live() -> Json<serde_json::Value> {
    Json(json!({"status":"ok"}))
}
async fn ready(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(Json(json!({"status":"ok"})))
}
async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../../docs/openapi.json"),
    )
}

async fn security(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let cross_site = request
        .headers()
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        == Some("cross-site");
    let mutating = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if path.starts_with("/api/")
        && (cross_site
            || origin.is_some_and(|o| o != state.config.app_origin)
            || (mutating && origin != Some(state.config.app_origin.as_str())))
    {
        return response_headers(&state, AppError::Forbidden.into_response(), true);
    }
    if [
        "/api/v1/auth/register",
        "/api/v1/auth/login",
        "/api/v1/auth/refresh",
        "/api/v1/auth/csrf",
        "/api/v1/users/password",
    ]
    .contains(&path)
    {
        // Only the actual socket peer is trusted. Forwarded headers are client-controlled.
        let ip = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|p| p.0.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        if let Err(error) =
            state
                .rate_limiter
                .check(ip, path, state.config.auth_rate_limit_per_minute)
        {
            return response_headers(&state, error.into_response(), true);
        }
    }
    let api = path.starts_with("/api/");
    let mut response = next.run(request).await;
    if api
        && [
            StatusCode::BAD_REQUEST,
            StatusCode::METHOD_NOT_ALLOWED,
            StatusCode::PAYLOAD_TOO_LARGE,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            StatusCode::REQUEST_TIMEOUT,
        ]
        .contains(&response.status())
    {
        let status = response.status();
        response = (status,Json(json!({"error":{"code":"INVALID_REQUEST","message":"Некорректный запрос или превышен лимит запроса","details":{}}}))).into_response();
    }
    response_headers(&state, response, api)
}

fn response_headers(state: &AppState, mut response: Response, api: bool) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "content-security-policy",
        state.content_security_policy.clone(),
    );
    if state.config.production {
        headers.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    if api {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

pub fn router(state: AppState) -> Router {
    let allowed_origin = HeaderValue::from_str(&state.config.app_origin).ok();
    let mut cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-csrf-token"),
        ]);
    if let Some(origin) = allowed_origin {
        cors = cors.allow_origin(origin);
    }
    let api = auth::router()
        .merge(finance::router())
        .route("/health", get(ready))
        .route("/openapi.json", get(openapi))
        .fallback(not_found);
    let files = ServeDir::new(&state.config.frontend_dir)
        .not_found_service(ServeFile::new(state.config.frontend_dir.join("index.html")));
    let mut app = Router::new()
        .nest("/api/v1", api)
        .route("/api/{*rest}", any(not_found))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready));
    // Known CSR routes need HTTP 200 on direct navigation. Unknown paths keep 404
    // while rendering the client Not Found page through ServeDir's fallback.
    for path in [
        "/",
        "/login",
        "/register",
        "/transactions",
        "/transactions/new",
        "/categories",
        "/budgets",
        "/recurring",
        "/analytics",
        "/profile",
    ] {
        app = app.route_service(
            path,
            ServeFile::new(state.config.frontend_dir.join("index.html")),
        );
    }
    app.fallback_service(files)
        .layer(DefaultBodyLimit::max(65_536))
        .layer(ConcurrencyLimitLayer::new(128))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(cors)
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// SQLx coordinates concurrent startup migrations with PostgreSQL advisory locking.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
