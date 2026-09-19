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
                .unwrap_or_else(|_| "chantels_corner=info,tower_http=info".into()),
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
    let addr = listen_address(
        std::env::var("ADDR").ok().as_deref(),
        std::env::var("PORT").ok().as_deref(),
    )?;

    tracing::info!("{} listening on http://{}", brand::STORE_NAME, addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn listen_address(
    addr: Option<&str>,
    port: Option<&str>,
) -> Result<SocketAddr, std::net::AddrParseError> {
    let address = match (addr, port) {
        (Some(addr), _) => addr.to_string(),
        (None, Some(port)) => format!("0.0.0.0:{port}"),
        (None, None) => "127.0.0.1:8080".to_string(),
    };
    address.parse()
}

#[cfg(test)]
mod tests {
    use super::listen_address;

    #[test]
    fn railway_port_binds_all_interfaces() {
        assert_eq!(
            listen_address(None, Some("3000")).unwrap().to_string(),
            "0.0.0.0:3000"
        );
    }

    #[test]
    fn explicit_address_overrides_port() {
        assert_eq!(
            listen_address(Some("127.0.0.1:8081"), Some("3000"))
                .unwrap()
                .to_string(),
            "127.0.0.1:8081"
        );
    }

    #[test]
    fn local_default_remains_unchanged() {
        assert_eq!(
            listen_address(None, None).unwrap().to_string(),
            "127.0.0.1:8080"
        );
        assert!(listen_address(None, Some("invalid")).is_err());
    }
}
