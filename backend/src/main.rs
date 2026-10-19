use backend::{AppState, config::Config};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, time::Duration};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env().map_err(std::io::Error::other)?;
    let pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;
    if config.run_migrations {
        backend::migrate(&pool).await?;
    }
    let address = SocketAddr::new(config.host, config.port);
    if config.production && !config.frontend_dir.join("index.html").is_file() {
        return Err("Production frontend bundle is missing; build with Trunk first".into());
    }
    let state = AppState::new(pool.clone(), config).await?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
    let worker_pool = pool.clone();
    let scheduler = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match backend::services::process_due(&worker_pool,chrono::Utc::now().date_naive()).await {
                        Ok(count) if count>0 => tracing::info!(count,"recurring transactions generated"),
                        Err(error) => tracing::error!(%error,"recurring scheduler failed"),
                        _ => {}
                    }
                    if let Err(error) = sqlx::query("DELETE FROM refresh_sessions WHERE expires_at < now() - interval '7 days'").execute(&worker_pool).await { tracing::error!(%error,"session cleanup failed"); }
                }
                _ = stop_rx.changed() => break,
            }
        }
    });
    tracing::info!(%address,"SpendWise server listening");
    let served = axum::serve(
        listener,
        backend::router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await;
    let _ = stop_tx.send(true);
    let _ = scheduler.await;
    pool.close().await;
    served?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error,"signal listener failed");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => tracing::error!(%error,"SIGTERM listener failed"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("Graceful shutdown requested");
}
