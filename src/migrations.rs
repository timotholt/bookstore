//! Startup migration policy. See docs/PRODUCTION_MIGRATIONS_SPEC.md.
//!
//! Locally the app role owns the database and applies migrations at startup.
//! In production the app role is deliberately not the owner, so the web
//! service only verifies the ledger and the operator applies migrations as
//! the owner with `chantels-corner --migrate`.

use std::collections::HashMap;

use sqlx::migrate::Migrator;

use crate::db::DbPool;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations_postgres");

const SPEC: &str = "docs/PRODUCTION_MIGRATIONS_SPEC.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Apply,
    Verify,
}

impl Mode {
    /// Parses `DATABASE_MIGRATIONS`; unset means `apply`.
    pub fn from_env_value(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim) {
            None | Some("") | Some("apply") => Ok(Mode::Apply),
            Some("verify") => Ok(Mode::Verify),
            Some(other) => Err(format!(
                "DATABASE_MIGRATIONS must be 'apply' or 'verify', got '{other}'"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Pending(i64),
    ChecksumMismatch(i64),
    Failed(i64),
}

/// Compares the ledger with the migrations embedded in this binary. Applied
/// versions this binary does not know are tolerated: during an operator
/// migration the previous build keeps running against the newer schema.
pub async fn problems(db: &DbPool, migrator: &Migrator) -> Result<Vec<Problem>, sqlx::Error> {
    let ledger_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
            .fetch_one(db)
            .await?;
    let applied: HashMap<i64, (Vec<u8>, bool)> = if ledger_exists {
        sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
            "SELECT version, checksum, success FROM _sqlx_migrations",
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|(version, checksum, success)| (version, (checksum, success)))
        .collect()
    } else {
        HashMap::new()
    };
    Ok(migrator
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .filter_map(|m| match applied.get(&m.version) {
            None => Some(Problem::Pending(m.version)),
            Some((_, false)) => Some(Problem::Failed(m.version)),
            Some((checksum, true)) if checksum.as_slice() != &*m.checksum => {
                Some(Problem::ChecksumMismatch(m.version))
            }
            Some(_) => None,
        })
        .collect())
}

/// Read-only startup check used in production.
pub async fn verify(db: &DbPool) -> Result<(), String> {
    let found = problems(db, &MIGRATOR)
        .await
        .map_err(|err| format!("could not read the migration ledger: {err}"))?;
    if found.is_empty() {
        return Ok(());
    }
    let list = found
        .iter()
        .map(|p| match p {
            Problem::Pending(v) => format!("{v} pending"),
            Problem::ChecksumMismatch(v) => format!("{v} checksum mismatch"),
            Problem::Failed(v) => format!("{v} marked failed"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "database migrations are not applied ({list}). Apply them as the database owner \
         with `chantels-corner --migrate` before deploying; see {SPEC}"
    ))
}

/// The owner connection must be named explicitly; never fall back to the
/// app's DATABASE_URL.
pub fn owner_url(value: Option<String>) -> Result<String, String> {
    match value {
        Some(url) if url.starts_with("postgres:") || url.starts_with("postgresql:") => Ok(url),
        Some(url) if !url.trim().is_empty() => {
            Err("MIGRATION_DATABASE_URL must use postgres:// or postgresql://".into())
        }
        _ => Err(format!(
            "--migrate requires MIGRATION_DATABASE_URL (the database owner connection); see {SPEC}"
        )),
    }
}

/// `chantels-corner --migrate`: applies embedded migrations as the owner.
pub async fn apply_as_owner() -> Result<(), Box<dyn std::error::Error>> {
    let url = owner_url(std::env::var("MIGRATION_DATABASE_URL").ok())?;
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await?;
    let pending = problems(&db, &MIGRATOR).await?;
    MIGRATOR.run(&db).await?;
    verify(&db).await?;
    println!(
        "Migrations up to date ({} applied by this run).",
        pending
            .iter()
            .filter(|p| matches!(p, Problem::Pending(_)))
            .count()
    );
    db.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_defaults_to_apply_and_rejects_unknown_values() {
        assert_eq!(Mode::from_env_value(None), Ok(Mode::Apply));
        assert_eq!(Mode::from_env_value(Some("")), Ok(Mode::Apply));
        assert_eq!(Mode::from_env_value(Some("apply")), Ok(Mode::Apply));
        assert_eq!(Mode::from_env_value(Some(" verify ")), Ok(Mode::Verify));
        assert!(Mode::from_env_value(Some("skip")).is_err());
    }

    #[test]
    fn owner_url_is_required_and_must_be_postgres() {
        assert!(owner_url(None)
            .unwrap_err()
            .contains("MIGRATION_DATABASE_URL"));
        assert!(owner_url(Some("  ".into())).is_err());
        assert!(owner_url(Some("mysql://x/db".into())).is_err());
        assert!(owner_url(Some("postgres://owner@127.0.0.1/db".into())).is_ok());
    }
}
