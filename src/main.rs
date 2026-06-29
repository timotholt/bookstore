use sqlx::sqlite::SqlitePool;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod models;
mod errors;
mod templates;
mod store;
mod handlers;
mod ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env configuration
    let _ = dotenvy::dotenv();

    // Set up logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "davis_books=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database connection setup
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/bookstore.db?mode=rwc".to_string());
    
    // Ensure parent directory for database exists
    if let Some(path) = std::path::Path::new(&db_url.trim_start_matches("sqlite://")).parent() {
        if !path.as_os_str().is_empty() {
            std::fs::create_dir_all(path)?;
        }
    }

    let db = SqlitePool::connect(&db_url).await?;

    // Run pending database migrations
    sqlx::migrate!("./migrations").run(&db).await?;
    tracing::info!("Database migrations executed successfully");

    let app = app::build_router(app::AppState { db });

    // Bind and start the server
    let addr_str = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = addr_str.parse()?;
    
    tracing::info!("Davis's Books listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
