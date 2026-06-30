use crate::env_loader::{EnvStore, EnvValue};
use crate::manifest::SetupManifest;
use crate::report::{Finding, Report};
use serde_json::Value;
use std::fs;
use std::path::Path;
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
    root: &Path,
    manifest: &SetupManifest,
    env_store: &EnvStore,
    apply: bool,
) -> Report {
    let mut report = Report::new("neon.setup");
    let Some(neon) = manifest.provider("neon") else {
        report.findings.push(Finding::fail(
            "neon.manifest",
            "neon",
            "missing",
            "setup/setup.toml does not declare a Neon provider.",
            "",
            "manifest_incomplete",
            "Add a [[providers]] entry with id = \"neon\".",
        ));
        return report;
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

    report.findings.push(Finding::info(
        "neon.desired_state",
        "neon",
        "planned",
        "Neon desired state loaded from setup/setup.toml.",
        &format!(
            "project={project}; database={database}; branches={branch_summary}; roles={role_summary}"
        ),
    ));

    if usable_env_value(env_store, "NEON_API_KEY").is_none() {
        report.findings.push(Finding::warn(
            "neon.credentials",
            "neon",
            "missing",
            "Neon setup cannot inspect or rebuild provider resources without NEON_API_KEY.",
            "Add NEON_API_KEY to setup/.secrets.demo.env or import it from a recovery email with `cargo xtask external secrets import-email --from <path> --yes`.",
        ));
        return report;
    }

    if !command_exists("curl") {
        report.findings.push(Finding::warn(
            "neon.api.account",
            "neon",
            "blocked",
            "Neon setup needs curl, but curl is not installed.",
            "Install curl or run the tool on a machine with curl.",
        ));
        return report;
    }

    if !apply {
        report.findings.push(Finding::warn(
            "neon.apply",
            "neon",
            "dry_run",
            "Would inspect and create missing Neon resources through the Neon API.",
            "Re-run with `cargo xtask external setup --yes` or `cargo xtask external repair --only neon --yes` to apply.",
        ));
        return report;
    }

    match apply_neon_setup(root, manifest, env_store) {
        Ok(mut findings) => report.findings.append(&mut findings),
        Err(err) => report.findings.push(Finding::fail(
            "neon.apply",
            "neon",
            "failed",
            "Neon setup failed.",
            &err,
            "provider_apply_failed",
            "Inspect the Neon API error, update setup/setup.toml or credentials, then rerun setup.",
        )),
    }

    report
}

fn apply_neon_setup(
    root: &Path,
    manifest: &SetupManifest,
    env_store: &EnvStore,
) -> Result<Vec<Finding>, String> {
    let api_key = usable_env_value(env_store, "NEON_API_KEY")
        .ok_or_else(|| "NEON_API_KEY is missing".to_string())?;
    let neon = manifest
        .provider("neon")
        .ok_or_else(|| "setup/setup.toml does not declare a Neon provider".to_string())?;
    let project_name = neon.value("project").unwrap_or("davis-books");
    let database_name = neon.value("database").unwrap_or("davis_books");
    let branch_name = neon
        .array("branches")
        .first()
        .map(String::as_str)
        .unwrap_or("main");
    let role_name = neon
        .array("roles")
        .first()
        .map(String::as_str)
        .unwrap_or("davis_books_app");

    let mut findings = Vec::new();
    let project = ensure_neon_project(
        api_key,
        neon,
        project_name,
        branch_name,
        database_name,
        role_name,
    )?;
    findings.push(project.finding);

    let branch = ensure_neon_branch(api_key, &project.id, branch_name)?;
    findings.push(branch.finding);

    let role = ensure_neon_role(api_key, &project.id, &branch.id, role_name)?;
    findings.push(role.finding);

    let database =
        ensure_neon_database(api_key, &project.id, &branch.id, database_name, role_name)?;
    findings.push(database.finding);

    let uri = get_neon_connection_uri(api_key, &project.id, &branch.id, database_name, role_name)?;
    write_demo_secret(root, "DATABASE_URL", &uri)?;
    findings.push(Finding::ok(
        "neon.database_url.write",
        "neon",
        "written",
        "Wrote Neon DATABASE_URL to setup/.secrets.demo.env.",
        "Value redacted.",
    ));

    Ok(findings)
}

struct EnsuredResource {
    id: String,
    finding: Finding,
}

fn ensure_neon_project(
    api_key: &EnvValue,
    neon: &crate::manifest::ProviderManifest,
    project_name: &str,
    branch_name: &str,
    database_name: &str,
    role_name: &str,
) -> Result<EnsuredResource, String> {
    let org_id = neon.value("org_id");
    let query = match org_id {
        Some(org_id) if !org_id.trim().is_empty() => format!(
            "/projects?limit=400&search={}&org_id={}",
            url_encode(project_name),
            url_encode(org_id)
        ),
        _ => format!("/projects?limit=400&search={}", url_encode(project_name)),
    };
    let projects = neon_get_json(&api_key.value, &query)?;
    if let Some(id) = find_named_object_id(&projects, "projects", project_name) {
        return Ok(EnsuredResource {
            id,
            finding: Finding::ok(
                "neon.project",
                "neon",
                "present",
                "Expected Neon project exists.",
                &format!("project={project_name}"),
            ),
        });
    }

    let mut project = serde_json::json!({
        "name": project_name,
        "store_passwords": true,
        "branch": {
            "name": branch_name,
            "database_name": database_name,
            "role_name": role_name
        }
    });
    if let Some(region) = neon.value("region_id") {
        project["region_id"] = Value::String(region.to_string());
    }
    if let Some(org_id) = org_id {
        project["org_id"] = Value::String(org_id.to_string());
    }

    let created = neon_post_json(
        &api_key.value,
        "/projects",
        &serde_json::json!({ "project": project }),
    )?;
    let id = created
        .get("project")
        .and_then(|project| project.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Neon project create response did not include project.id".to_string())?
        .to_string();

    Ok(EnsuredResource {
        id,
        finding: Finding::ok(
            "neon.project",
            "neon",
            "created",
            "Created Neon project from setup/setup.toml.",
            &format!("project={project_name}"),
        ),
    })
}

fn ensure_neon_branch(
    api_key: &EnvValue,
    project_id: &str,
    branch_name: &str,
) -> Result<EnsuredResource, String> {
    let branches = neon_get_json(&api_key.value, &format!("/projects/{project_id}/branches"))?;
    if let Some(id) = find_named_object_id(&branches, "branches", branch_name) {
        return Ok(EnsuredResource {
            id,
            finding: Finding::ok(
                "neon.branch",
                "neon",
                "present",
                "Expected Neon branch exists.",
                &format!("branch={branch_name}"),
            ),
        });
    }

    let created = neon_post_json(
        &api_key.value,
        &format!("/projects/{project_id}/branches"),
        &serde_json::json!({
            "branch": { "name": branch_name },
            "endpoints": [{ "type": "read_write" }]
        }),
    )?;
    let id = created
        .get("branch")
        .and_then(|branch| branch.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Neon branch create response did not include branch.id".to_string())?
        .to_string();

    Ok(EnsuredResource {
        id,
        finding: Finding::ok(
            "neon.branch",
            "neon",
            "created",
            "Created Neon branch from setup/setup.toml.",
            &format!("branch={branch_name}"),
        ),
    })
}

fn ensure_neon_role(
    api_key: &EnvValue,
    project_id: &str,
    branch_id: &str,
    role_name: &str,
) -> Result<EnsuredResource, String> {
    let roles = neon_get_json(
        &api_key.value,
        &format!("/projects/{project_id}/branches/{branch_id}/roles"),
    )?;
    if let Some(id) = find_named_object_id(&roles, "roles", role_name) {
        return Ok(EnsuredResource {
            id,
            finding: Finding::ok(
                "neon.role",
                "neon",
                "present",
                "Expected Neon role exists.",
                &format!("role={role_name}"),
            ),
        });
    }

    let created = neon_post_json(
        &api_key.value,
        &format!("/projects/{project_id}/branches/{branch_id}/roles"),
        &serde_json::json!({ "role": { "name": role_name } }),
    )?;
    let id = created
        .get("role")
        .and_then(|role| role.get("name"))
        .and_then(Value::as_str)
        .unwrap_or(role_name)
        .to_string();

    Ok(EnsuredResource {
        id,
        finding: Finding::ok(
            "neon.role",
            "neon",
            "created",
            "Created Neon role from setup/setup.toml.",
            &format!("role={role_name}"),
        ),
    })
}

fn ensure_neon_database(
    api_key: &EnvValue,
    project_id: &str,
    branch_id: &str,
    database_name: &str,
    role_name: &str,
) -> Result<EnsuredResource, String> {
    let databases = neon_get_json(
        &api_key.value,
        &format!("/projects/{project_id}/branches/{branch_id}/databases"),
    )?;
    if let Some(id) = find_named_object_id(&databases, "databases", database_name) {
        return Ok(EnsuredResource {
            id,
            finding: Finding::ok(
                "neon.database",
                "neon",
                "present",
                "Expected Neon database exists.",
                &format!("database={database_name}"),
            ),
        });
    }

    let created = neon_post_json(
        &api_key.value,
        &format!("/projects/{project_id}/branches/{branch_id}/databases"),
        &serde_json::json!({
            "database": {
                "name": database_name,
                "owner_name": role_name
            }
        }),
    )?;
    let id = created
        .get("database")
        .map(|database| object_identifier(database, database_name))
        .unwrap_or_else(|| database_name.to_string());

    Ok(EnsuredResource {
        id,
        finding: Finding::ok(
            "neon.database",
            "neon",
            "created",
            "Created Neon database from setup/setup.toml.",
            &format!("database={database_name}"),
        ),
    })
}

fn get_neon_connection_uri(
    api_key: &EnvValue,
    project_id: &str,
    branch_id: &str,
    database_name: &str,
    role_name: &str,
) -> Result<String, String> {
    let path = format!(
        "/projects/{project_id}/connection_uri?branch_id={}&database_name={}&role_name={}&pooled=true",
        url_encode(branch_id),
        url_encode(database_name),
        url_encode(role_name)
    );
    let json = neon_get_json(&api_key.value, &path)?;
    json.get("uri")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "Neon connection URI response did not include uri".to_string())
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

    let project_query = match neon.value("org_id") {
        Some(org_id) if !org_id.trim().is_empty() => format!(
            "/projects?limit=400&search={}&org_id={}",
            url_encode(project_name),
            url_encode(org_id)
        ),
        _ => format!("/projects?limit=400&search={}", url_encode(project_name)),
    };
    let projects = match neon_get_json(&api_key.value, &project_query) {
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

fn neon_post_json(api_key: &str, path: &str, body: &Value) -> Result<Value, String> {
    let url = format!("https://console.neon.tech/api/v2{path}");
    let body = serde_json::to_string(body).map_err(|err| format!("invalid JSON body: {err}"))?;
    let output = Command::new("curl")
        .args([
            "-sS",
            "--fail",
            "-X",
            "POST",
            "-H",
            "Accept: application/json",
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("Authorization: Bearer {api_key}"),
            "--data",
            &body,
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

fn write_demo_secret(root: &Path, key: &str, value: &str) -> Result<(), String> {
    let path = root.join("setup/.secrets.demo.env");
    let mut lines = match fs::read_to_string(&path) {
        Ok(contents) => contents
            .lines()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(format!("failed to read {}: {err}", path.display())),
    };

    let mut replaced = false;
    for line in &mut lines {
        let trimmed = line.trim_start();
        let exported = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        if exported
            .split_once('=')
            .map(|(name, _)| name.trim() == key)
            .unwrap_or(false)
        {
            *line = format!("{key}={}", shell_quote_env_value(value));
            replaced = true;
        }
    }
    if !replaced {
        lines.push(format!("{key}={}", shell_quote_env_value(value)));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&path, format!("{}\n", lines.join("\n")))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn shell_quote_env_value(value: &str) -> String {
    if value.bytes().all(|byte| {
        matches!(
            byte,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'_'
                | b'-'
                | b'.'
                | b':'
                | b'/'
                | b'?'
                | b'='
                | b'&'
                | b'%'
                | b'@'
        )
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
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
            findings.push(Finding::ok(
                "neon.app_runtime",
                "neon",
                "postgres",
                "The app runtime can select Postgres from DATABASE_URL.",
                "src/main.rs uses sqlx AnyPool and migrations_postgres for Postgres URLs.",
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
    use crate::report::Severity;
    use std::collections::BTreeMap;

    #[test]
    fn neon_reports_sqlite_url_blocker() {
        let store = store_with([("DATABASE_URL", "sqlite://data/bookstore.db?mode=rwc")]);
        let findings = validate_neon_readiness(&store);
        assert!(findings
            .iter()
            .any(|finding| finding.id == "neon.app_runtime"));
    }

    #[test]
    fn neon_reports_postgres_runtime_ready() {
        let store = store_with([("DATABASE_URL", "postgres://redacted")]);
        let findings = validate_neon_readiness(&store);
        assert!(findings.iter().any(|finding| {
            finding.id == "neon.app_runtime" && finding.severity == Severity::Ok
        }));
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
            std::path::Path::new("."),
            &manifest,
            &store_with([("DATABASE_URL", "postgres://redacted")]),
            false,
        )
        .findings;

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

    #[test]
    fn writes_demo_secret_without_duplicate_keys() {
        let root = temp_dir("write_demo_secret");
        fs::create_dir_all(root.join("setup")).unwrap();
        fs::write(
            root.join("setup/.secrets.demo.env"),
            "NEON_API_KEY=redacted\nDATABASE_URL=old\n",
        )
        .unwrap();

        write_demo_secret(&root, "DATABASE_URL", "postgresql://new?sslmode=require").unwrap();
        let contents = fs::read_to_string(root.join("setup/.secrets.demo.env")).unwrap();

        assert_eq!(contents.matches("DATABASE_URL=").count(), 1);
        assert!(contents.contains("DATABASE_URL=postgresql://new?sslmode=require"));

        let _ = fs::remove_dir_all(root);
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

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("davis_books_providers_{name}_{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
