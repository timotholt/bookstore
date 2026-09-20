use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod account_email;
mod app;
mod auth;
mod brand;
mod cart;
mod db;
mod email;
mod errors;
mod handlers;
mod models;
mod store;
mod templates;
mod ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--render-email-previews") {
        render_email_previews()?;
        return Ok(());
    }
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

    let email = std::sync::Arc::new(email::EmailService::from_env(
        std::env::var("APP_ENV").unwrap_or_default() == "production",
    )?);
    let _email_worker = email.clone().spawn_worker(db.clone());
    let app = app::build_router(app::AppState { db, email });

    // Bind and start the server
    let addr = listen_address(
        std::env::var("ADDR").ok().as_deref(),
        std::env::var("PORT").ok().as_deref(),
    )?;

    tracing::info!("{} listening on http://{}", brand::STORE_NAME, addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

fn render_email_previews() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::Path::new("docs/email-previews");
    std::fs::create_dir_all(output)?;
    for (name, kind) in [
        ("verification", email::EmailKind::Verification),
        ("password-reset", email::EmailKind::PasswordReset),
        ("password-changed", email::EmailKind::PasswordChanged),
    ] {
        let (html, plain) = email::render_preview(
            kind,
            "#preview-only",
            "../../assets/email/chantels-corner-header.jpg",
        )?;
        std::fs::write(output.join(format!("{name}.html")), html)?;
        std::fs::write(output.join(format!("{name}.txt")), plain)?;
    }
    println!("Email previews written to docs/email-previews; no email sent.");
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
