use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod auth;
mod brand;
mod cart;
mod db;
mod errors;
mod handlers;
mod models;
mod store;
mod templates;
mod ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    db::load_runtime_env();

    // Set up logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "davis_books=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = db::require_database_url()?;
    db::require_postgres_url(&db_url)?;
    let db = db::connect(&db_url).await?;

    // Run pending database migrations
    sqlx::migrate!("./migrations_postgres").run(&db).await?;
    tracing::info!("Database migrations executed successfully");

    let session_store = tower_sessions_sqlx_store::PostgresStore::new(db.clone());
    session_store.migrate().await?;
    tracing::info!("Session store migrated successfully");

    let app = app::build_router(app::AppState { db });

    // Bind and start the server
    let addr_str = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = addr_str.parse()?;

    tracing::info!("{} listening on http://{}", brand::STORE_NAME, addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
