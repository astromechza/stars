pub mod assets;
pub mod auth;
pub mod calendar;
pub mod config;
pub mod handlers;
pub mod store;
pub mod templates;

use config::Config;
use handlers::AppState;
use store::Store;

pub async fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = Config::from_env();
    let store = Store::connect(&cfg.database_url)
        .await
        .expect("failed to connect/migrate database");
    let state = AppState {
        store,
        dev_user: cfg.dev_user.clone(),
    };

    let app = handlers::router(state);
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .expect("failed to bind");
    tracing::info!("listening on {}", cfg.bind_addr);
    axum::serve(listener, app).await.expect("server error");
}
