use axum::{
    routing::{get, post},
    Router,
};
use sqlx::sqlite::SqlitePool;
use std::net::SocketAddr;
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::{SessionManagerLayer, MemoryStore};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    // Configure session management (MemoryStore matches Go SCS session store)
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(std::env::var("APP_ENV").unwrap_or_default() == "production")
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    // Axum Router registration
    let app = Router::new()
        .route("/", get(handlers::home))
        .route("/catalog", get(handlers::catalog))
        .route("/books/:book_id", get(handlers::book_detail))
        .route("/cart", get(handlers::cart_page))
        .route("/cart/items", post(handlers::add_cart_item))
        .route("/cart/items/:copy_id/increase", post(handlers::increase_cart_item))
        .route("/cart/items/:copy_id/decrease", post(handlers::decrease_cart_item))
        .route("/cart/items/:copy_id/remove", post(handlers::remove_cart_item))
        .route("/checkout", post(handlers::checkout))
        .nest_service("/assets", ServeDir::new("assets"))
        .route_service("/app.js", ServeFile::new("app.js"))
        .route_service("/styles.css", ServeFile::new("styles.css"))
        .layer(session_layer)
        .with_state(db);

    // Bind and start the server
    let addr_str = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = addr_str.parse()?;
    
    tracing::info!("Davis's Books listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
