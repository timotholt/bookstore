use crate::env_loader::EnvStore;
use crate::manifest::SetupManifest;
use crate::report::Finding;

pub fn validate_provider_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(validate_neon_readiness(env_store));
    findings.extend(validate_railway_readiness(env_store));
    findings.extend(validate_stripe_readiness(env_store));
    findings
}

pub fn plan_neon_setup(
    manifest: &SetupManifest,
    env_store: &EnvStore,
    apply: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let Some(neon) = manifest.provider("neon") else {
        findings.push(Finding::fail(
            "neon.manifest",
            "neon",
            "missing",
            "setup/setup.toml does not declare a Neon provider.",
            "",
            "manifest_incomplete",
            "Add a [[providers]] entry with id = \"neon\".",
        ));
        return findings;
    };

    let project = neon.value("project").unwrap_or("davis-books");
    let database = neon.value("database").unwrap_or("davis_books");
    let branches = neon.array("branches");
    let roles = neon.array("roles");
    let branch_summary = if branches.is_empty() {
        "main".to_string()
    } else {
        branches.join(", ")
    };
    let role_summary = if roles.is_empty() {
        "davis_books_app, davis_books_migrator".to_string()
    } else {
        roles.join(", ")
    };

    findings.push(Finding::info(
        "neon.desired_state",
        "neon",
        "planned",
        "Neon desired state loaded from setup/setup.toml.",
        &format!(
            "project={project}; database={database}; branches={branch_summary}; roles={role_summary}"
        ),
    ));

    if env_store.get("NEON_API_KEY").is_some() {
        findings.push(Finding::manual(
            "neon.api.inspect",
            "neon",
            if apply { "manual_apply" } else { "planned" },
            "Neon API mutation is not enabled yet, but credentials are available for the next adapter slice.",
            &format!(
                "Validate or create project `{project}`, database `{database}`, branches `{branch_summary}`, and roles `{role_summary}` in Neon. Future adapter should perform this through the Neon API."
            ),
        ));
    } else {
        findings.push(Finding::warn(
            "neon.credentials",
            "neon",
            "missing",
            "Neon setup cannot inspect or rebuild provider resources without NEON_API_KEY.",
            "Add NEON_API_KEY to setup/.secrets.demo.env or import it from a recovery email with `cargo xtask external secrets import-email --from <path> --yes`.",
        ));
    }

    match env_store.get("DATABASE_URL") {
        Some(value)
            if value.value.starts_with("postgres:") || value.value.starts_with("postgresql:") =>
        {
            findings.push(Finding::manual(
                "neon.database.validate",
                "neon",
                "pending_adapter",
                "A Postgres DATABASE_URL is present, but Postgres migration validation is not implemented yet.",
                "Next slice: add a Postgres database validator that checks reachability and migrations_postgres/ status.",
            ));
        }
        Some(value) if value.value.starts_with("sqlite:") => {
            findings.push(Finding::fail(
                "neon.database_url",
                "neon",
                "blocked",
                "Neon setup cannot validate the database while DATABASE_URL points at SQLite.",
                "DATABASE_URL is sqlite.",
                "postgres_database_url_missing",
                "Set DATABASE_URL to a Neon postgres:// URL when validating external database state.",
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
                "Use a postgres:// Neon URL for external database validation.",
            ));
        }
        None => findings.push(Finding::warn(
            "neon.database_url",
            "neon",
            "missing",
            "DATABASE_URL is missing.",
            "Add a Neon postgres:// DATABASE_URL to setup/.secrets.demo.env before running external database validation.",
        )),
    }

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
            findings.push(Finding::fail(
                "neon.app_runtime",
                "neon",
                "pending",
                "Neon database validation can run, but the app runtime still uses SqlitePool.",
                "src/main.rs and src/app.rs are SQLite-specific.",
                "postgres_runtime_missing",
                "Port the app runtime and SQL query modules before treating Neon as deploy-ready.",
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
    use crate::manifest::{ProviderManifest, SetupManifest};
    use std::collections::BTreeMap;

    #[test]
    fn neon_reports_sqlite_runtime_blocker() {
        let store = store_with([("DATABASE_URL", "sqlite://data/bookstore.db?mode=rwc")]);
        let findings = validate_neon_readiness(&store);
        assert!(findings
            .iter()
            .any(|finding| finding.id == "neon.app_runtime"));
    }

    #[test]
    fn neon_setup_reports_manifest_state() {
        let mut provider = ProviderManifest {
            id: "neon".to_string(),
            adapter: "api".to_string(),
            required: false,
            values: BTreeMap::new(),
            arrays: BTreeMap::new(),
        };
        provider
            .values
            .insert("project".to_string(), "davis-books".to_string());
        provider
            .values
            .insert("database".to_string(), "davis_books".to_string());
        provider.arrays.insert(
            "roles".to_string(),
            vec!["app".to_string(), "migrator".to_string()],
        );
        let manifest = SetupManifest {
            project: BTreeMap::new(),
            providers: vec![provider],
        };

        let findings = plan_neon_setup(
            &manifest,
            &store_with([("DATABASE_URL", "postgres://redacted")]),
            false,
        );

        assert!(findings.iter().any(|finding| {
            finding.id == "neon.desired_state" && finding.evidence.contains("davis_books")
        }));
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
