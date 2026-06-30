use sqlx::any::{install_default_drivers, AnyPoolOptions};
use std::io::{Error, ErrorKind};

pub type DbPool = sqlx::AnyPool;
pub type Db = sqlx::Any;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Postgres,
    Sqlite,
}

pub fn install_drivers() {
    install_default_drivers();
}

pub fn require_database_url() -> Result<String, Error> {
    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        Error::new(
            ErrorKind::InvalidInput,
            "DATABASE_URL is required; set it explicitly to a sqlite:// or postgres:// URL",
        )
    })?;

    if database_url.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "DATABASE_URL is empty; set it explicitly to a sqlite:// or postgres:// URL",
        ));
    }

    Ok(database_url)
}

pub fn database_kind(database_url: &str) -> Result<DatabaseKind, Error> {
    if database_url.starts_with("sqlite:") {
        Ok(DatabaseKind::Sqlite)
    } else if database_url.starts_with("postgres:") || database_url.starts_with("postgresql:") {
        Ok(DatabaseKind::Postgres)
    } else {
        Err(Error::new(
            ErrorKind::InvalidInput,
            "DATABASE_URL must use sqlite://, sqlite:, postgres://, or postgresql://",
        ))
    }
}

pub fn ensure_sqlite_parent(database_url: &str) -> std::io::Result<()> {
    if database_kind(database_url)? != DatabaseKind::Sqlite {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_kind_accepts_explicit_supported_urls() {
        assert_eq!(
            database_kind("sqlite://data/bookstore.db?mode=rwc").unwrap(),
            DatabaseKind::Sqlite
        );
        assert_eq!(
            database_kind("postgresql://example/db").unwrap(),
            DatabaseKind::Postgres
        );
    }

    #[test]
    fn database_kind_rejects_unsupported_urls() {
        assert!(database_kind("").is_err());
        assert!(database_kind("mysql://example/db").is_err());
    }
}
