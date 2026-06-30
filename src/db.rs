use sqlx::any::{install_default_drivers, AnyPoolOptions};

pub type DbPool = sqlx::AnyPool;
pub type Db = sqlx::Any;

pub fn install_drivers() {
    install_default_drivers();
}

pub fn default_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/bookstore.db?mode=rwc".into())
}

pub fn is_sqlite_url(database_url: &str) -> bool {
    database_url.starts_with("sqlite:")
}

pub fn is_postgres_url(database_url: &str) -> bool {
    database_url.starts_with("postgres:") || database_url.starts_with("postgresql:")
}

pub fn ensure_sqlite_parent(database_url: &str) -> std::io::Result<()> {
    if !is_sqlite_url(database_url) {
        return Ok(());
    }

    let path = database_url
        .trim_start_matches("sqlite://")
        .trim_start_matches("sqlite:");
    if path == ":memory:" || path.is_empty() {
        return Ok(());
    }

    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

pub async fn connect(database_url: &str) -> Result<DbPool, sqlx::Error> {
    AnyPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}
