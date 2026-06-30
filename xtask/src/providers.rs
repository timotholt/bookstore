use crate::env_loader::{EnvStore, EnvValue};
use crate::manifest::SetupManifest;
use crate::report::Finding;
use serde_json::Value;
use std::process::{Command, Stdio};

pub fn validate_provider_readiness(
    manifest: Option<&SetupManifest>,
    env_store: &EnvStore,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(validate_neon_readiness(env_store));
    if let Some(manifest) = manifest {
        findings.extend(validate_neon_api(manifest, env_store));
    }
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

    if usable_env_value(env_store, "NEON_API_KEY").is_some() {
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

fn validate_neon_api(manifest: &SetupManifest, env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();
    let Some(api_key) = usable_env_value(env_store, "NEON_API_KEY") else {
        findings.push(Finding::skipped(
            "neon.api.account",
            "neon",
            "blocked",
            "Neon API resource validation skipped because NEON_API_KEY is missing.",
            "required_env_missing",
        ));
        return findings;
    };
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

    if !command_exists("curl") {
        findings.push(Finding::warn(
            "neon.api.account",
            "neon",
            "blocked",
            "Neon API validation needs curl, but curl is not installed.",
            "Install curl or use the local-only validation path.",
        ));
        return findings;
    }

    let project_name = neon.value("project").unwrap_or("davis-books");
    let database_name = neon.value("database").unwrap_or("davis_books");
    let branch_name = neon
        .array("branches")
        .first()
        .map(String::as_str)
        .unwrap_or("main");

    let projects = match neon_get_json(
        &api_key.value,
        &format!("/projects?limit=400&search={}", url_encode(project_name)),
    ) {
        Ok(json) => json,
        Err(err) => {
            findings.push(Finding::fail(
                "neon.api.account",
                "neon",
                "failed",
                "Could not query the Neon projects API.",
                &err,
                "provider_api_failed",
                "Verify NEON_API_KEY, network access, and Neon account access.",
            ));
            return findings;
        }
    };

    let Some(project_id) = find_named_object_id(&projects, "projects", project_name) else {
        findings.push(Finding::fail(
            "neon.project",
            "neon",
            "missing",
            "Expected Neon project was not found.",
            &format!("project={project_name}"),
            "provider_resource_missing",
            "Create the Neon project or update setup/setup.toml to match the existing project name.",
        ));
        findings.push(Finding::skipped(
            "neon.branch",
            "neon",
            "blocked",
            "Branch validation skipped because the Neon project is missing.",
            "prerequisite_failed",
        ));
        return findings;
    };

    findings.push(Finding::ok(
        "neon.project",
        "neon",
        "present",
        "Expected Neon project exists.",
        &format!("project={project_name}; project_id={project_id}"),
    ));

    let branches = match neon_get_json(&api_key.value, &format!("/projects/{project_id}/branches"))
    {
        Ok(json) => json,
        Err(err) => {
            findings.push(Finding::fail(
                "neon.branch",
                "neon",
                "failed",
                "Could not query Neon branches.",
                &err,
                "provider_api_failed",
                "Verify the Neon project ID and API token permissions.",
            ));
            return findings;
        }
    };

    let Some(branch_id) = find_named_object_id(&branches, "branches", branch_name) else {
        findings.push(Finding::fail(
            "neon.branch",
            "neon",
            "missing",
            "Expected Neon branch was not found.",
            &format!("branch={branch_name}"),
            "provider_resource_missing",
            "Create the expected Neon branch or update setup/setup.toml.",
        ));
        findings.push(Finding::skipped(
            "neon.database",
            "neon",
            "blocked",
            "Database validation skipped because the Neon branch is missing.",
            "prerequisite_failed",
        ));
        return findings;
    };

    findings.push(Finding::ok(
        "neon.branch",
        "neon",
        "present",
        "Expected Neon branch exists.",
        &format!("branch={branch_name}; branch_id={branch_id}"),
    ));

    findings.extend(validate_neon_branch_collection(
        &api_key.value,
        &project_id,
        &branch_id,
        "databases",
        database_name,
        "neon.database",
        "database",
    ));

    for role in neon.array("roles") {
        findings.extend(validate_neon_branch_collection(
            &api_key.value,
            &project_id,
            &branch_id,
            "roles",
            role,
            "neon.role",
            "role",
        ));
    }

    findings
}

fn validate_neon_branch_collection(
    api_key: &str,
    project_id: &str,
    branch_id: &str,
    collection: &'static str,
    expected_name: &str,
    id: &'static str,
    label: &'static str,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let path = format!("/projects/{project_id}/branches/{branch_id}/{collection}");
    match neon_get_json(api_key, &path) {
        Ok(json) if find_named_object_id(&json, collection, expected_name).is_some() => {
            findings.push(Finding::ok(
                id,
                "neon",
                "present",
                &format!("Expected Neon {label} exists."),
                &format!("{label}={expected_name}"),
            ));
        }
        Ok(_) => findings.push(Finding::fail(
            id,
            "neon",
            "missing",
            &format!("Expected Neon {label} was not found."),
            &format!("{label}={expected_name}"),
            "provider_resource_missing",
            &format!("Create Neon {label} `{expected_name}` or update setup/setup.toml."),
        )),
        Err(err) => findings.push(Finding::fail(
            id,
            "neon",
            "failed",
            &format!("Could not query Neon {collection}."),
            &err,
            "provider_api_failed",
            "Verify the Neon project, branch, and API token permissions.",
        )),
    }
    findings
}

fn neon_get_json(api_key: &str, path: &str) -> Result<Value, String> {
    let url = format!("https://console.neon.tech/api/v2{path}");
    let output = Command::new("curl")
        .args([
            "-sS",
            "--fail",
            "-H",
            "Accept: application/json",
            "-H",
            &format!("Authorization: Bearer {api_key}"),
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("failed to run curl: {err}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    serde_json::from_slice(&output.stdout).map_err(|err| format!("invalid Neon API JSON: {err}"))
}

fn find_named_object_id(json: &Value, collection: &str, expected: &str) -> Option<String> {
    json.get(collection)?
        .as_array()?
        .iter()
        .find(|item| object_matches_name_or_id(item, expected))
        .map(|item| object_identifier(item, expected))
}

fn object_identifier(item: &Value, fallback: &str) -> String {
    for key in ["id", "project_id", "branch_id", "name"] {
        if let Some(value) = item.get(key) {
            if let Some(value) = value.as_str() {
                return value.to_string();
            }
            if let Some(value) = value.as_i64() {
                return value.to_string();
            }
            if let Some(value) = value.as_u64() {
                return value.to_string();
            }
        }
    }
    fallback.to_string()
}

fn object_matches_name_or_id(item: &Value, expected: &str) -> bool {
    ["name", "id", "project_id", "branch_id"]
        .iter()
        .filter_map(|key| item.get(key))
        .filter_map(Value::as_str)
        .any(|value| value == expected)
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            byte => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
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

fn validate_neon_readiness(env_store: &EnvStore) -> Vec<Finding> {
    let mut findings = Vec::new();

    if usable_env_value(env_store, "NEON_API_KEY").is_some() {
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

    #[test]
    fn neon_resource_lookup_finds_names_and_ids() {
        let json: Value = serde_json::json!({
            "projects": [
                {"id": "silent", "name": "other"},
                {"id": "project-id", "name": "davis-books"}
            ]
        });

        assert_eq!(
            find_named_object_id(&json, "projects", "davis-books"),
            Some("project-id".to_string())
        );
    }

    #[test]
    fn neon_resource_lookup_handles_numeric_ids() {
        let json: Value = serde_json::json!({
            "databases": [
                {"id": 22264968, "name": "neondb"}
            ]
        });

        assert_eq!(
            find_named_object_id(&json, "databases", "neondb"),
            Some("22264968".to_string())
        );
    }

    #[test]
    fn url_encode_handles_spaces_and_symbols() {
        assert_eq!(url_encode("davis books/test"), "davis%20books%2Ftest");
    }

    #[test]
    fn placeholder_values_are_not_usable_credentials() {
        let store = store_with([("NEON_API_KEY", "replace-with-neon-key")]);

        assert!(usable_env_value(&store, "NEON_API_KEY").is_none());
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
