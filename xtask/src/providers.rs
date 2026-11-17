use crate::env_loader::{EnvStore, EnvValue};
use crate::report::Finding;

pub fn validate_provider_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(validate_database_url_readiness(env_store));
    findings.extend(validate_railway_readiness(env_store));
    findings.extend(validate_stripe_readiness(env_store));
    findings
}

fn usable_env_value<'a>(env_store: &'a EnvStore, key: &str) -> Option<&'a EnvValue> {
    env_store
        .get(key)
        .filter(|value| !is_placeholder_secret(&value.value))
}

fn is_placeholder_secret(value: &str) -> bool {
    let value = value.trim();
    value.is_empty()
        || value.eq_ignore_ascii_case("replace-me")
        || value.starts_with("replace-with-")
        || value.ends_with("_replace_me")
        || value.contains("_replace_")
}

fn validate_database_url_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();

    match env_store.get("DATABASE_URL") {
        Some(value)
            if value.value.starts_with("postgres:") || value.value.starts_with("postgresql:") =>
        {
            findings.push(Finding::ok(
                "database.url",
                "database",
                "postgres",
                "DATABASE_URL is Postgres-compatible.",
                "Value redacted.",
            ));
            findings.push(Finding::ok(
                "database.app_runtime",
                "database",
                "postgres",
                "The app runtime is configured for Postgres.",
                "src/main.rs uses PgPool and migrations_postgres.",
            ));
        }
        Some(value) if value.value.starts_with("sqlite:") => {
            findings.push(Finding::fail(
                "database.app_runtime",
                "database",
                "unsupported",
                "SQLite DATABASE_URL is not supported by the app runtime.",
                "DATABASE_URL is sqlite.",
                "postgres_required",
                "Use an explicit postgres:// or postgresql:// DATABASE_URL.",
            ));
        }
        Some(_) => {
            findings.push(Finding::fail(
                "database.url",
                "database",
                "invalid",
                "DATABASE_URL is not Postgres-compatible.",
                "Value redacted.",
                "database_url_unsupported",
                "Use a postgres:// or postgresql:// URL.",
            ));
        }
        None => {
            findings.push(Finding::skipped(
                "database.url",
                "database",
                "blocked",
                "DATABASE_URL readiness skipped because DATABASE_URL is missing.",
                "required_env_missing",
            ));
        }
    }

    findings
}

fn validate_railway_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    if usable_env_value(env_store, "RAILWAY_TOKEN").is_some() {
        findings.push(Finding::ok(
            "railway.credentials",
            "railway",
            "present",
            "RAILWAY_TOKEN is present in recognized env sources.",
            "Value redacted.",
        ));
    } else {
        findings.push(Finding::warn(
            "railway.credentials",
            "railway",
            "missing",
            "RAILWAY_TOKEN is missing.",
            "Add RAILWAY_TOKEN to .env before enabling Railway API setup.",
        ));
    }

    if env_store.get("PUBLIC_BASE_URL").is_some() {
        findings.push(Finding::ok(
            "railway.public_base_url",
            "railway",
            "present",
            "PUBLIC_BASE_URL is present for deployed route validation.",
            "Value redacted.",
        ));
    } else {
        findings.push(Finding::warn(
            "railway.public_base_url",
            "railway",
            "missing",
            "PUBLIC_BASE_URL is missing.",
            "Set PUBLIC_BASE_URL after Railway creates a public domain.",
        ));
    }

    findings
}

fn validate_stripe_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    if usable_env_value(env_store, "STRIPE_SECRET_KEY").is_some() {
        findings.push(Finding::ok(
            "stripe.credentials",
            "stripe",
            "present",
            "STRIPE_SECRET_KEY is present in recognized env sources.",
            "Value redacted.",
        ));
    } else {
        findings.push(Finding::warn(
            "stripe.credentials",
            "stripe",
            "missing",
            "STRIPE_SECRET_KEY is missing.",
            "Stripe is optional until checkout is implemented.",
        ));
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_loader::{EnvSource, EnvStore, EnvValue};
    use crate::report::Severity;
    use std::collections::BTreeMap;

    #[test]
    fn database_url_rejects_sqlite() {
        let store = store_with([("DATABASE_URL", "sqlite://data/bookstore.db?mode=rwc")]);
        let findings = validate_database_url_readiness(&store);
        assert!(findings.iter().any(|finding| {
            finding.id == "database.app_runtime"
                && finding.severity == Severity::Fail
                && finding.cause == "postgres_required"
        }));
    }

    #[test]
    fn database_url_reports_postgres_runtime_ready() {
        let store = store_with([("DATABASE_URL", "postgres://redacted")]);
        let findings = validate_database_url_readiness(&store);
        assert!(findings.iter().any(|finding| {
            finding.id == "database.app_runtime" && finding.severity == Severity::Ok
        }));
    }

    #[test]
    fn placeholder_values_are_not_usable_credentials() {
        let store = store_with([("RAILWAY_TOKEN", "replace-with-railway-token")]);

        assert!(usable_env_value(&store, "RAILWAY_TOKEN").is_none());
    }

    fn store_with<const N: usize>(pairs: [(&str, &str); N]) -> EnvStore {
        let mut values = BTreeMap::new();
        for (name, value) in pairs {
            values.insert(
                name.to_string(),
                EnvValue {
                    value: value.to_string(),
                    source: EnvSource::Process,
                },
            );
        }
        EnvStore::from_values(values)
    }
}
