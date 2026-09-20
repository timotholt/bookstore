use std::collections::HashSet;
use std::io::{Error, ErrorKind};

pub type DbPool = sqlx::PgPool;
pub type Db = sqlx::Postgres;

pub fn load_runtime_env() {
    let process_keys = std::env::vars()
        .map(|(name, _)| name)
        .collect::<HashSet<_>>();

    for path in [".env"] {
        let Ok(iter) = dotenvy::from_path_iter(path) else {
            continue;
        };

        for item in iter.flatten() {
            let (name, value) = item;
            if !process_keys.contains(&name) {
                std::env::set_var(name, value);
            }
        }
    }
}

pub fn require_database_url() -> Result<String, Error> {
    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        Error::new(
            ErrorKind::InvalidInput,
            "DATABASE_URL is required; set it explicitly to a postgres:// or postgresql:// URL",
        )
    })?;

    if database_url.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "DATABASE_URL is empty; set it explicitly to a postgres:// or postgresql:// URL",
        ));
    }

    Ok(database_url)
}

pub fn require_postgres_url(database_url: &str) -> Result<(), Error> {
    if database_url.starts_with("postgres:") || database_url.starts_with("postgresql:") {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::InvalidInput,
            "DATABASE_URL must use postgres:// or postgresql://",
        ))
    }
}

pub async fn connect(database_url: &str) -> Result<DbPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_postgres_url_accepts_postgres_urls() {
        assert!(require_postgres_url("postgres://example/db").is_ok());
        assert!(require_postgres_url("postgresql://example/db").is_ok());
    }

    #[test]
    fn require_postgres_url_rejects_non_postgres_urls() {
        assert!(require_postgres_url("").is_err());
        assert!(require_postgres_url("sqlite://data/bookstore.db").is_err());
        assert!(require_postgres_url("mysql://example/db").is_err());
    }
}
