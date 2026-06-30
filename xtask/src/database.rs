use crate::env_loader::EnvStore;
use crate::report::{Finding, Report};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn validate_database(root: &Path, env_store: &EnvStore) -> Report {
    let mut report = Report::new("database.validate");
    let Some(database_url) = env_store.get("DATABASE_URL") else {
        report.findings.push(Finding::skipped(
            "database.connection",
            "database",
            "blocked",
            "Database validation skipped because DATABASE_URL is missing.",
            "required_env_missing",
        ));
        return report;
    };

    if !database_url.value.starts_with("sqlite:") {
        report.findings.push(Finding::manual(
            "database.connection",
            "database",
            "unsupported",
            "Only SQLite database validation is implemented in this slice.",
            "Postgres/Neon validation should reuse this adapter shape and add a Postgres connection path.",
        ));
        return report;
    }

    match run_sqlite_validation(root, &database_url.value) {
        Ok(findings) => report.findings.extend(findings),
        Err(err) => report.findings.push(Finding::fail(
            "database.connection",
            "database",
            "failed",
            "Could not validate the SQLite database.",
            &err,
            "database_validation_failed",
            "Check DATABASE_URL, create the local DB with cargo run, or run app migrations.",
        )),
    }

    report
}

fn run_sqlite_validation(root: &Path, database_url: &str) -> Result<Vec<Finding>, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to create tokio runtime: {err}"))?;
    runtime.block_on(validate_sqlite_async(root, database_url))
}

async fn validate_sqlite_async(root: &Path, database_url: &str) -> Result<Vec<Finding>, String> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .map_err(|err| format!("connect failed: {err}"))?;

    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .map_err(|err| format!("could not query _sqlx_migrations: {err}"))?;

    let expected = migration_versions(root)?;
    let applied_set: BTreeSet<i64> = applied.into_iter().collect();
    let missing: Vec<i64> = expected
        .iter()
        .copied()
        .filter(|version| !applied_set.contains(version))
        .collect();

    let mut findings = Vec::new();
    findings.push(Finding::ok(
        "database.connection",
        "database",
        "reachable",
        "Connected to SQLite DATABASE_URL.",
        "Connection string redacted.",
    ));

    if missing.is_empty() {
        findings.push(Finding::ok(
            "database.migrations.applied",
            "database",
            "current",
            &format!("All {} SQL migrations are applied.", expected.len()),
            "Compared migrations/ filenames against _sqlx_migrations.",
        ));
    } else {
        findings.push(Finding::fail(
            "database.migrations.applied",
            "database",
            "missing",
            "The database is missing one or more migrations.",
            &format!("Missing versions: {:?}", missing),
            "migrations_not_applied",
            "Run the app migration path or `sqlx migrate run` against DATABASE_URL.",
        ));
    }

    Ok(findings)
}

fn migration_versions(root: &Path) -> Result<Vec<i64>, String> {
    let mut versions = Vec::new();
    for entry in fs::read_dir(root.join("migrations"))
        .map_err(|err| format!("could not read migrations directory: {err}"))?
    {
        let entry = entry.map_err(|err| format!("could not read migration entry: {err}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.ends_with(".sql") {
            continue;
        }
        let Some((version, _)) = name.split_once('_') else {
            continue;
        };
        if let Ok(version) = version.parse::<i64>() {
            versions.push(version);
        }
    }
    versions.sort_unstable();
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn migration_versions_reads_sqlx_filename_prefixes() {
        let dir = temp_dir("migration_versions");
        fs::create_dir_all(dir.join("migrations")).unwrap();
        fs::write(dir.join("migrations/20260629000000_schema.sql"), "").unwrap();
        fs::write(dir.join("migrations/20260629001000_next.sql"), "").unwrap();
        fs::write(dir.join("migrations/not-a-migration.txt"), "").unwrap();

        assert_eq!(
            migration_versions(&dir).unwrap(),
            vec![20260629000000, 20260629001000]
        );

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("davis_books_xtask_{name}_{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
