use crate::env_loader::EnvStore;
use crate::report::Finding;

pub fn validate_provider_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(validate_neon_readiness(env_store));
    findings.extend(validate_railway_readiness(env_store));
    findings.extend(validate_stripe_readiness(env_store));
    findings
}

fn validate_neon_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();

    if env_store.get("NEON_API_KEY").is_some() {
        findings.push(Finding::ok(
            "neon.credentials",
            "neon",
            "present",
            "NEON_API_KEY is present in recognized env sources.",
            "Value redacted.",
        ));
    } else {
        findings.push(Finding::warn(
            "neon.credentials",
            "neon",
            "missing",
            "NEON_API_KEY is missing.",
            "Add NEON_API_KEY to setup/.secrets.demo.env before enabling Neon API setup.",
        ));
    }

    match env_store.get("DATABASE_URL") {
        Some(value)
            if value.value.starts_with("postgres:") || value.value.starts_with("postgresql:") =>
        {
            findings.push(Finding::ok(
                "neon.database_url",
                "neon",
                "postgres",
                "DATABASE_URL is Postgres-compatible.",
                "Value redacted.",
            ));
        }
        Some(value) if value.value.starts_with("sqlite:") => {
            findings.push(Finding::fail(
                "neon.app_runtime",
                "neon",
                "blocked",
                "Neon setup is blocked by the current SQLite runtime path.",
                "DATABASE_URL is sqlite.",
                "postgres_runtime_missing",
                "Add Postgres runtime support before treating Neon as deploy-ready.",
            ));
        }
        Some(_) => {
            findings.push(Finding::fail(
                "neon.database_url",
                "neon",
                "invalid",
                "DATABASE_URL is neither sqlite nor postgres.",
                "Value redacted.",
                "database_url_unsupported",
                "Use a sqlite:// local URL or postgres:// Neon URL.",
            ));
        }
        None => {
            findings.push(Finding::skipped(
                "neon.database_url",
                "neon",
                "blocked",
                "Neon DATABASE_URL readiness skipped because DATABASE_URL is missing.",
                "required_env_missing",
            ));
        }
    }

    findings
}

fn validate_railway_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    if env_store.get("RAILWAY_TOKEN").is_some() {
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
            "Add RAILWAY_TOKEN to setup/.secrets.demo.env before enabling Railway API setup.",
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
    if env_store.get("STRIPE_SECRET_KEY").is_some() {
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
    use std::collections::BTreeMap;

    #[test]
    fn neon_reports_sqlite_runtime_blocker() {
        let store = store_with([("DATABASE_URL", "sqlite://data/bookstore.db?mode=rwc")]);
        let findings = validate_neon_readiness(&store);
        assert!(findings
            .iter()
            .any(|finding| finding.id == "neon.app_runtime"));
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
