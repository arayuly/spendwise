use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::{AppState, config::Config};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

#[tokio::test]
async fn direct_spa_routes_return_success_and_unknown_routes_keep_not_found() {
    let directory =
        std::env::temp_dir().join(format!("spendwise-static-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(
        directory.join("index.html"),
        "<!doctype html><html lang=ru><title>SpendWise test</title></html>",
    )
    .unwrap();
    let pool = PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/unused_static_test")
        .unwrap();
    let config = Config {
        database_url: String::new(),
        jwt_secret: "static-test-only-0123456789abcdefghijklmnopqrstuvwxyz".into(),
        app_origin: "http://localhost:8080".into(),
        host: "127.0.0.1".parse().unwrap(),
        port: 8080,
        cookie_secure: false,
        production: false,
        frontend_dir: directory.clone(),
        run_migrations: false,
        access_token_minutes: 10,
        refresh_token_days: 30,
        database_max_connections: 1,
        auth_rate_limit_per_minute: 20,
    };
    let app = backend::router(AppState::new(pool.clone(), config).await.unwrap());
    for (path, status) in [
        ("/", StatusCode::OK),
        ("/transactions", StatusCode::OK),
        ("/transactions/new", StatusCode::OK),
        ("/analytics", StatusCode::OK),
        ("/unknown-client-route", StatusCode::NOT_FOUND),
        ("/missing.wasm", StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    }
    pool.close().await;
    drop(app);
    std::fs::remove_file(directory.join("index.html")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
