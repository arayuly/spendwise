//! End-to-end HTTP tests against a fresh SQLx-managed PostgreSQL database per test.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use backend::{AppState, config::Config};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const ORIGIN: &str = "http://localhost:8080";
async fn app(pool: PgPool) -> Router {
    let config = Config {
        database_url: String::new(),
        jwt_secret: "integration-test-secret-ONLY-0123456789abcdefghijklmnopqrstuvwxyz".into(),
        app_origin: ORIGIN.into(),
        host: "127.0.0.1".parse().unwrap(),
        port: 8080,
        cookie_secure: false,
        production: false,
        frontend_dir: "frontend/dist".into(),
        run_migrations: true,
        access_token_minutes: 10,
        refresh_token_days: 30,
        database_max_connections: 10,
        auth_rate_limit_per_minute: 20,
    };
    backend::router(AppState::new(pool, config).await.unwrap())
}
struct Reply {
    status: StatusCode,
    json: Value,
    text: String,
    cookie: String,
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> Reply {
    let mut request = Request::builder()
        .method(method)
        .uri(format!("/api/v1{path}"))
        .header(header::ORIGIN, ORIGIN);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    let body = if let Some(body) = body {
        request = request.header(header::CONTENT_TYPE, "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_owned();
    let text = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Reply {
        status,
        json,
        text,
        cookie,
    }
}
struct Session {
    token: String,
    csrf: String,
    cookie: String,
    id: String,
}
async fn register(app: &Router, email: &str) -> Session {
    let r = call(
        app,
        "POST",
        "/auth/register",
        Some(json!({"email":email,"password":"A strong password 123!","display_name":"Тест"})),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    assert!(r.cookie.starts_with("spendwise_refresh="));
    Session {
        token: r.json["access_token"].as_str().unwrap().into(),
        csrf: r.json["csrf_token"].as_str().unwrap().into(),
        cookie: r.cookie,
        id: r.json["user"]["id"].as_str().unwrap().into(),
    }
}
async fn categories(app: &Router, session: &Session) -> Vec<Value> {
    let r = call(
        app,
        "GET",
        "/categories",
        None,
        Some(&session.token),
        None,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    r.json.as_array().unwrap().clone()
}
fn input(category: &Value, amount: &str, title: &str) -> Value {
    json!({"category_id":category["id"],"transaction_type":category["transaction_type"],"amount":amount,"title":title,"description":"Запись с запятой, и \"кавычками\"","transaction_date":"2026-01-15"})
}

#[sqlx::test(migrations = "./migrations")]
async fn ownership_financial_totals_budgets_and_csv(pool: PgPool) {
    let app = app(pool).await;
    let alice = register(&app, "alice@example.com").await;
    let bob = register(&app, "bob@example.com").await;
    let a = categories(&app, &alice).await;
    let b = categories(&app, &bob).await;
    assert_eq!(a.len(), 13);
    let expense = a
        .iter()
        .find(|c| c["transaction_type"] == "expense")
        .unwrap();
    let income = a
        .iter()
        .find(|c| c["transaction_type"] == "income")
        .unwrap();
    let created = call(
        &app,
        "POST",
        "/transactions",
        Some(input(expense, "25.50", "=SUM(A1:A2)")),
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    let id = created.json["id"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            "/transactions",
            Some(input(income, "100.00", "Доход")),
            Some(&alice.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/transactions",
            Some(input(&b[0], "777.00", "Bob private")),
            Some(&bob.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::CREATED
    );
    for method in ["GET", "PATCH", "DELETE"] {
        let body = (method == "PATCH").then(|| input(&b[0], "1.00", "stolen"));
        assert_eq!(
            call(
                &app,
                method,
                &format!("/transactions/{id}"),
                body,
                Some(&bob.token),
                None,
                None
            )
            .await
            .status,
            StatusCode::NOT_FOUND
        );
    }
    let budget = call(
        &app,
        "POST",
        "/budgets",
        Some(json!({"category_id":expense["id"],"month":"2026-01-01","amount":"50.00"})),
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(budget.status, StatusCode::CREATED, "{}", budget.text);
    assert_eq!(
        budget.json["spent"]
            .as_str()
            .unwrap()
            .parse::<rust_decimal::Decimal>()
            .unwrap(),
        rust_decimal::Decimal::new(255, 1)
    );
    let summary = call(
        &app,
        "GET",
        "/analytics/summary?from=2026-01-01&to=2026-01-31",
        None,
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(summary.status, StatusCode::OK, "{}", summary.text);
    for (field, expected) in [
        ("income", "100"),
        ("expenses", "25.50"),
        ("savings", "74.50"),
        ("balance", "74.50"),
    ] {
        assert_eq!(
            summary.json[field]
                .as_str()
                .unwrap()
                .parse::<rust_decimal::Decimal>()
                .unwrap(),
            expected.parse::<rust_decimal::Decimal>().unwrap()
        );
    }
    let months = call(
        &app,
        "GET",
        "/analytics/monthly?from=2026-01-01&to=2026-01-31",
        None,
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(months.status, StatusCode::OK);
    assert!(!months.json.as_array().unwrap().is_empty());
    let csv = call(
        &app,
        "GET",
        "/export/transactions?from=2026-01-01&to=2026-01-31",
        None,
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.text);
    assert!(!csv.text.contains("Bob private"));
    assert!(csv.text.contains("'=SUM(A1:A2)"));
    assert!(csv.text.contains("кавычками"));
    let all = call(
        &app,
        "GET",
        "/transactions?page=1&per_page=1",
        None,
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(all.json["total"], 2);
    assert_eq!(all.json["items"].as_array().unwrap().len(), 1);
    let updated = call(
        &app,
        "PATCH",
        &format!("/transactions/{id}"),
        Some(input(expense, "30.00", "Обновлено")),
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text);
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/transactions/{id}"),
            None,
            Some(&alice.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    let archive = call(
        &app,
        "DELETE",
        &format!("/categories/{}", income["id"].as_str().unwrap()),
        None,
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert_eq!(archive.status, StatusCode::NO_CONTENT);
    assert_eq!(
        call(
            &app,
            "POST",
            "/transactions",
            Some(input(income, "1.00", "archived")),
            Some(&alice.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn validation_csrf_origin_and_currency_guards(pool: PgPool) {
    let app = app(pool).await;
    let alice = register(&app, "validation@example.com").await;
    let bob = register(&app, "other@example.com").await;
    let cats = categories(&app, &alice).await;
    let foreign = categories(&app, &bob).await;
    for amount in ["0", "-1", "0.001", "10000000000000000", "NaN"] {
        let r = call(
            &app,
            "POST",
            "/transactions",
            Some(input(&cats[0], amount, "invalid")),
            Some(&alice.token),
            None,
            None,
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "amount {amount}: {}",
            r.text
        );
        assert_eq!(r.json["error"]["code"], "VALIDATION_ERROR");
    }
    let other = call(
        &app,
        "POST",
        "/transactions",
        Some(input(&foreign[0], "1.00", "foreign")),
        Some(&alice.token),
        None,
        None,
    )
    .await;
    assert!([StatusCode::NOT_FOUND, StatusCode::UNPROCESSABLE_ENTITY].contains(&other.status));
    assert_eq!(
        call(&app, "GET", "/transactions", None, None, None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/unknown-route", None, None, None, None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/refresh",
            None,
            None,
            Some(&alice.cookie),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let raw = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::COOKIE, &alice.cookie)
        .header("x-csrf-token", &alice.csrf)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(raw).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            "/users/me",
            Some(json!({"display_name":"Тест","currency":"USD"})),
            Some(&alice.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/transactions",
            Some(input(&cats[0], "1.00", "valid")),
            Some(&alice.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            "/users/me",
            Some(json!({"display_name":"Тест","currency":"EUR"})),
            Some(&alice.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_ne!(alice.id, bob.id);
}

#[sqlx::test(migrations = "./migrations")]
async fn rotating_sessions_logout_and_password_revocation(pool: PgPool) {
    let app = app(pool).await;
    let session = register(&app, "session@example.com").await;
    let invalid = format!("spendwise_refresh={}", "a".repeat(43));
    assert_eq!(
        call(&app, "GET", "/auth/csrf", None, None, Some(&invalid), None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let bootstrap = call(
        &app,
        "GET",
        "/auth/csrf",
        None,
        None,
        Some(&session.cookie),
        None,
    )
    .await;
    assert_eq!(bootstrap.json["csrf_token"], session.csrf);
    let rotated = call(
        &app,
        "POST",
        "/auth/refresh",
        None,
        None,
        Some(&session.cookie),
        Some(&session.csrf),
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK, "{}", rotated.text);
    assert_ne!(rotated.cookie, session.cookie);
    assert_eq!(
        call(
            &app,
            "GET",
            "/auth/me",
            None,
            Some(&session.token),
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let token = rotated.json["access_token"].as_str().unwrap();
    let csrf = rotated.json["csrf_token"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/logout",
            None,
            Some(token),
            Some(&rotated.cookie),
            Some(csrf)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/auth/me", None, Some(token), None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/refresh",
            None,
            None,
            Some(&rotated.cookie),
            Some(csrf)
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let logged = call(
        &app,
        "POST",
        "/auth/login",
        Some(json!({"email":"session@example.com","password":"A strong password 123!"})),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(logged.status, StatusCode::OK);
    let token = logged.json["access_token"].as_str().unwrap();
    let changed=call(&app,"POST","/users/password",Some(json!({"current_password":"A strong password 123!","new_password":"Another strong password!"})),Some(token),None,None).await;
    assert_eq!(changed.status, StatusCode::NO_CONTENT, "{}", changed.text);
    assert_eq!(
        call(&app, "GET", "/auth/me", None, Some(token), None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/login",
            Some(json!({"email":"session@example.com","password":"A strong password 123!"})),
            None,
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn replay_and_logout_all_revoke_every_device(pool: PgPool) {
    let app = app(pool).await;
    let first = register(&app, "replay@example.com").await;
    let rotated = call(
        &app,
        "POST",
        "/auth/refresh",
        None,
        None,
        Some(&first.cookie),
        Some(&first.csrf),
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK);
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/refresh",
            None,
            None,
            Some(&first.cookie),
            Some(&first.csrf)
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/auth/me",
            None,
            Some(rotated.json["access_token"].as_str().unwrap()),
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let one = call(
        &app,
        "POST",
        "/auth/login",
        Some(json!({"email":"replay@example.com","password":"A strong password 123!"})),
        None,
        None,
        None,
    )
    .await;
    let two = call(
        &app,
        "POST",
        "/auth/login",
        Some(json!({"email":"replay@example.com","password":"A strong password 123!"})),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(two.status, StatusCode::OK);
    let result = call(
        &app,
        "POST",
        "/auth/logout-all",
        None,
        Some(one.json["access_token"].as_str().unwrap()),
        Some(&one.cookie),
        Some(one.json["csrf_token"].as_str().unwrap()),
    )
    .await;
    assert_eq!(result.status, StatusCode::NO_CONTENT, "{}", result.text);
    for token in [
        one.json["access_token"].as_str().unwrap(),
        two.json["access_token"].as_str().unwrap(),
    ] {
        assert_eq!(
            call(&app, "GET", "/auth/me", None, Some(token), None, None)
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn malformed_requests_security_headers_and_expired_access(pool: PgPool) {
    let app = app(pool).await;
    let session = register(&app, "headers@example.com").await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("x".repeat(65_537)))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let policy = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(
        !policy
            .split("; style-src")
            .next()
            .unwrap()
            .contains("unsafe-inline")
    );
    let mut claims: Value = serde_json::from_slice(
        &base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            session.token.split('.').nth(1).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    claims["exp"] = json!(1);
    let expired = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(
            b"integration-test-secret-ONLY-0123456789abcdefghijklmnopqrstuvwxyz",
        ),
    )
    .unwrap();
    assert_eq!(
        call(&app, "GET", "/auth/me", None, Some(&expired), None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::ORIGIN, ORIGIN)
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn recurring_generation_is_concurrent_and_idempotent(pool: PgPool) {
    let app = app(pool.clone()).await;
    let session = register(&app, "recurring@example.com").await;
    let cats = categories(&app, &session).await;
    let category = cats
        .iter()
        .find(|c| c["transaction_type"] == "expense")
        .unwrap();
    let recurring=call(&app,"POST","/recurring",Some(json!({"category_id":category["id"],"transaction_type":"expense","amount":"10.00","title":"Аренда","description":null,"next_date":"2026-01-31","active":true})),Some(&session.token),None,None).await;
    assert_eq!(recurring.status, StatusCode::CREATED, "{}", recurring.text);
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    let (one, two) = tokio::join!(
        backend::services::process_due(&pool, date),
        backend::services::process_due(&pool, date)
    );
    assert_eq!(one.unwrap() + two.unwrap(), 3);
    assert_eq!(
        backend::services::process_due(&pool, date).await.unwrap(),
        0
    );
    let dates: Vec<chrono::NaiveDate> = sqlx::query_scalar(
        "SELECT transaction_date FROM transactions WHERE user_id=$1 ORDER BY transaction_date",
    )
    .bind(uuid::Uuid::parse_str(&session.id).unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        dates.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["2026-01-31", "2026-02-28", "2026-03-31"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn logout_after_rotation_closes_device_family_and_preserves_other_device(pool: PgPool) {
    let app = app(pool).await;
    let first = register(&app, "family@example.com").await;
    let other = call(
        &app,
        "POST",
        "/auth/login",
        Some(json!({"email":"family@example.com","password":"A strong password 123!"})),
        None,
        None,
        None,
    )
    .await;
    let rotated = call(
        &app,
        "POST",
        "/auth/refresh",
        None,
        None,
        Some(&first.cookie),
        Some(&first.csrf),
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK);
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/logout",
            None,
            None,
            Some(&first.cookie),
            Some(&first.csrf)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/auth/me",
            None,
            Some(rotated.json["access_token"].as_str().unwrap()),
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/auth/me",
            None,
            Some(other.json["access_token"].as_str().unwrap()),
            None,
            None
        )
        .await
        .status,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn rate_limit_responses_have_security_headers(pool: PgPool) {
    let app = app(pool).await;
    for _ in 0..20 {
        assert_eq!(
            call(&app, "GET", "/auth/csrf", None, None, None, None)
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
    }
    let request = Request::builder()
        .uri("/api/v1/auth/csrf")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "60");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-frame-options"], "DENY");
}
