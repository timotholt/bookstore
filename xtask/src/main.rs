mod database;
mod env_loader;
mod manifest;
mod providers;
mod report;
mod secrets;

use database::validate_database;
use env_loader::EnvStore;
use manifest::SetupManifest;
use providers::{plan_neon_setup, validate_provider_readiness};
use report::{render_human_report, render_json_report, Finding, Report};
use secrets::import_email;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = repo_root()?;

    match args.as_slice() {
        [] => {
            print_help();
            Ok(())
        }
        [cmd] if cmd == "help" || cmd == "--help" || cmd == "-h" => {
            print_help();
            Ok(())
        }
        [scope, rest @ ..] if scope == "external" => run_external(&root, rest),
        _ => Err(format!("unknown command: {}", args.join(" "))),
    }
}

fn run_external(root: &Path, args: &[String]) -> Result<(), String> {
    match args {
        [] => {
            print_external_help();
            Ok(())
        }
        [cmd, flags @ ..] if cmd == "doctor" => {
            let options = Options::parse(flags)?;
            let report = build_doctor_report(root);
            finish_report(root, report, &options)
        }
        [cmd, flags @ ..] if cmd == "validate" => {
            let options = Options::parse(flags)?;
            let report = build_validation_report(root, options.local_only);
            finish_report(root, report, &options)
        }
        [cmd, flags @ ..] if cmd == "plan" => {
            let options = Options::parse(flags)?;
            let mut report = Report::new("external.plan");
            report.findings.push(Finding::info(
                "plan.mode",
                "external",
                "read_only",
                "Planning is read-only in this first provider-free slice.",
                "The report combines doctor and validation findings. Provider action planning begins with the Neon adapter.",
            ));
            report.extend(build_doctor_report(root));
            report.extend(build_validation_report(root, options.local_only));
            finish_report(root, report, &options)
        }
        [cmd, flags @ ..] if cmd == "install-deps" => {
            let options = Options::parse(flags)?;
            let mut report = build_install_report(root, options.yes)?;
            if !options.yes {
                report.findings.push(Finding::warn(
                    "install.apply",
                    "local",
                    "dry_run",
                    "No installers ran because --yes was not passed.",
                    "Re-run with `cargo xtask external install-deps --yes` to install missing supported dependencies.",
                ));
            }
            finish_report(root, report, &options)
        }
        [cmd, flags @ ..] if cmd == "setup" => {
            let options = Options::parse(flags)?;
            let mut report = Report::new("external.setup");

            report.findings.push(Finding::info(
                "setup.mode",
                "external",
                "planned",
                "Running first-slice setup orchestration.",
                "This setup pass installs supported local dependencies only with --yes or --install-deps, then validates local bootstrap state. Provider mutation adapters are intentionally not enabled yet.",
            ));

            report.extend(build_doctor_report(root));
            let env_store = EnvStore::load(root);
            let manifest = SetupManifest::load(root).ok();

            if options.yes || options.install_deps {
                report.extend(build_install_report(root, true)?);
            } else {
                report.findings.push(Finding::warn(
                    "setup.install_deps",
                    "local",
                    "skipped",
                    "Dependency installation skipped.",
                    "Use `cargo xtask external setup --install-deps` or `cargo xtask external install-deps --yes`.",
                ));
            }

            report.extend(build_validation_report(root, options.local_only));
            if options.local_only {
                report.findings.push(Finding::info(
                    "setup.providers",
                    "external",
                    "local_only",
                    "Provider setup skipped by --local-only.",
                    "Run without --local-only to inspect provider setup plans.",
                ));
            } else if let Some(manifest) = manifest {
                report
                    .findings
                    .extend(plan_neon_setup(&manifest, &env_store, options.yes));
                report.findings.push(Finding::manual(
                    "setup.providers.remaining",
                    "external",
                    "not_implemented",
                    "Railway and Stripe setup adapters are not implemented yet.",
                    "After Neon database validation is real, implement Railway environment/deploy validation, then Stripe webhook setup.",
                ));
            } else {
                report.findings.push(Finding::fail(
                    "setup.manifest",
                    "manifest",
                    "missing",
                    "Provider setup cannot run because setup/setup.toml could not be loaded.",
                    "",
                    "manifest_missing",
                    "Create or restore setup/setup.toml.",
                ));
            }

            finish_report(root, report, &options)
        }
        [cmd, flags @ ..] if cmd == "repair" => {
            let options = Options::parse(flags)?;
            let mut report = Report::new("external.repair");
            if options.only.is_empty() {
                report.findings.push(Finding::fail(
                    "repair.target",
                    "external",
                    "missing",
                    "Repair requires an explicit --only selector.",
                    "",
                    "repair_target_missing",
                    "Run `cargo xtask external repair --only <finding-or-provider>` after reviewing validation output.",
                ));
            } else {
                report.findings.push(Finding::manual(
                    "repair.adapters",
                    "external",
                    "not_implemented",
                    "Targeted repair adapters are not implemented yet.",
                    "Next repair work should start with database.migrations after the database adapter exists.",
                ));
            }
            finish_report(root, report, &options)
        }
        [cmd, subcmd, flags @ ..] if cmd == "secrets" && subcmd == "import-email" => {
            let options = Options::parse(flags)?;
            let Some(from) = options.from.as_deref() else {
                return Err(
                    "secrets import-email requires `--from <path-to-email-or-note>`".to_string(),
                );
            };
            let report = import_email(root, from, options.yes)?;
            finish_report(root, report, &options)
        }
        [cmd, ..] => Err(format!("unknown external command: {cmd}")),
    }
}

#[derive(Default)]
struct Options {
    json: bool,
    yes: bool,
    local_only: bool,
    install_deps: bool,
    write_report: bool,
    only: Vec<String>,
    from: Option<String>,
}

impl Options {
    fn parse(flags: &[String]) -> Result<Self, String> {
        let mut options = Options::default();
        let mut index = 0;
        while index < flags.len() {
            match flags[index].as_str() {
                "--json" => options.json = true,
                "--yes" | "-y" => options.yes = true,
                "--local-only" => options.local_only = true,
                "--install-deps" => options.install_deps = true,
                "--write-report" => options.write_report = true,
                "--from" => {
                    index += 1;
                    let Some(value) = flags.get(index) else {
                        return Err("--from requires a file path".to_string());
                    };
                    options.from = Some(value.to_string());
                }
                flag if flag.starts_with("--from=") => {
                    options.from = Some(flag.trim_start_matches("--from=").to_string());
                }
                "--only" => {
                    index += 1;
                    let Some(value) = flags.get(index) else {
                        return Err("--only requires a selector value".to_string());
                    };
                    options.only.push(value.to_string());
                }
                flag if flag.starts_with("--only=") => {
                    options
                        .only
                        .push(flag.trim_start_matches("--only=").to_string());
                }
                "--help" | "-h" => {
                    print_external_help();
                    std::process::exit(0);
                }
                unknown => return Err(format!("unknown flag: {unknown}")),
            }
            index += 1;
        }
        Ok(options)
    }
}

fn build_doctor_report(root: &Path) -> Report {
    let mut report = Report::new("external.doctor");

    check_file(
        &mut report,
        root,
        "Cargo.toml",
        "local.cargo_manifest",
        true,
    );
    check_file(
        &mut report,
        root,
        "docs/EXTERNAL_WORLD_BOOTSTRAP_SPEC.md",
        "local.bootstrap_spec",
        true,
    );
    check_file(
        &mut report,
        root,
        "setup/setup.toml",
        "local.setup_manifest",
        true,
    );
    check_file(
        &mut report,
        root,
        "setup/secrets.example.env",
        "local.secrets_example",
        true,
    );
    check_file(
        &mut report,
        root,
        ".cargo/config.toml",
        "local.cargo_alias",
        true,
    );

    for dependency in tool_dependencies() {
        check_command(&mut report, dependency);
    }

    check_gitignore(root, &mut report);
    check_env_files(root, &mut report);
    check_migrations(root, &mut report);

    report
}

fn build_validation_report(root: &Path, local_only: bool) -> Report {
    let mut report = Report::new(if local_only {
        "external.validate.local"
    } else {
        "external.validate"
    });

    check_file(
        &mut report,
        root,
        "setup/setup.toml",
        "manifest.present",
        true,
    );
    check_manifest_content(root, &mut report);
    check_migrations(root, &mut report);
    check_git_tracking(root, &mut report);
    let env_store = EnvStore::load(root);
    check_env_contract(root, &env_store, &mut report);
    report.skip_if_failed(
        "env.runtime",
        "validate.providers.blocked",
        "external",
        "Provider validation is blocked until required runtime env is complete.",
    );

    if local_only {
        report.extend(validate_database(root, &env_store));
        report.findings.push(Finding::info(
            "validate.scope",
            "external",
            "local_only",
            "External provider checks skipped by --local-only.",
            "Run without --local-only after provider adapters are implemented.",
        ));
    } else {
        report.extend(validate_database(root, &env_store));
        report
            .findings
            .extend(validate_provider_readiness(&env_store));
    }

    report
}

fn build_install_report(root: &Path, apply: bool) -> Result<Report, String> {
    let mut report = Report::new("external.install_deps");

    if apply {
        run_install_command(
            &mut report,
            root,
            "install.rust_dependencies",
            "cargo",
            &["fetch"],
            "Fetched Rust crate dependencies.",
        )?;
    } else {
        report.findings.push(Finding::info(
            "install.rust_dependencies",
            "cargo",
            "planned",
            "Would run `cargo fetch` to install Rust crate dependencies.",
            "Pass --yes to execute.",
        ));
    }

    for dependency in tool_dependencies().into_iter().filter(|dep| !dep.required) {
        if command_exists(dependency.command) {
            report.findings.push(Finding::ok(
                dependency.id,
                "tool",
                "present",
                &format!("{} is installed.", dependency.command),
                dependency.reason,
            ));
            continue;
        }

        match dependency.install_command {
            Some((program, args)) if apply => {
                run_install_command(
                    &mut report,
                    root,
                    dependency.id,
                    program,
                    args,
                    &format!("Installed optional dependency {}.", dependency.command),
                )?;
            }
            Some((program, args)) => {
                report.findings.push(Finding::warn(
                    dependency.id,
                    "tool",
                    "missing",
                    &format!("{} is not installed.", dependency.command),
                    &format!(
                        "Would run `{}`. Pass --yes to execute.",
                        shell_words(program, args)
                    ),
                ));
            }
            None => report.findings.push(Finding::manual(
                dependency.id,
                "tool",
                "missing",
                &format!("{} is not installed.", dependency.command),
                dependency.reason,
            )),
        }
    }

    Ok(report)
}

fn run_install_command(
    report: &mut Report,
    root: &Path,
    id: &'static str,
    program: &str,
    args: &[&str],
    ok_summary: &str,
) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("failed to run {}: {}", shell_words(program, args), err))?;

    if output.status.success() {
        report.findings.push(Finding::ok(
            id,
            "installer",
            "changed_or_verified",
            ok_summary,
            &format!("Command succeeded: {}", shell_words(program, args)),
        ));
    } else {
        report.findings.push(Finding::fail(
            id,
            "installer",
            "failed",
            &format!("Install command failed: {}", shell_words(program, args)),
            &String::from_utf8_lossy(&output.stderr),
            "installer_failed",
            "Inspect the command output, then rerun install-deps after fixing the local toolchain or network issue.",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ToolDependency {
    id: &'static str,
    command: &'static str,
    required: bool,
    reason: &'static str,
    install_command: Option<(&'static str, &'static [&'static str])>,
}

fn tool_dependencies() -> Vec<ToolDependency> {
    vec![
        ToolDependency {
            id: "tool.cargo",
            command: "cargo",
            required: true,
            reason: "Required to build and run the Rust app and xtask.",
            install_command: None,
        },
        ToolDependency {
            id: "tool.rustc",
            command: "rustc",
            required: true,
            reason: "Required by the Rust toolchain.",
            install_command: None,
        },
        ToolDependency {
            id: "tool.git",
            command: "git",
            required: true,
            reason: "Required for repo validation, commit checks, and deploy source checks.",
            install_command: None,
        },
        ToolDependency {
            id: "tool.curl",
            command: "curl",
            required: false,
            reason: "Recommended for HTTP smoke checks and provider API debugging.",
            install_command: None,
        },
        ToolDependency {
            id: "tool.sqlx",
            command: "sqlx",
            required: false,
            reason: "Recommended for migration management once external database setup starts.",
            install_command: Some((
                "cargo",
                &[
                    "install",
                    "sqlx-cli",
                    "--no-default-features",
                    "--features",
                    "sqlite,postgres,rustls",
                ],
            )),
        },
        ToolDependency {
            id: "tool.node",
            command: "node",
            required: false,
            reason: "Recommended for future Playwright/browser fallback and provider CLIs distributed through npm.",
            install_command: None,
        },
        ToolDependency {
            id: "tool.npm",
            command: "npm",
            required: false,
            reason: "Recommended for installing provider CLIs and Playwright when those adapters are enabled.",
            install_command: None,
        },
        ToolDependency {
            id: "tool.railway",
            command: "railway",
            required: false,
            reason: "Recommended once Railway adapter is implemented.",
            install_command: Some(("npm", &["install", "-g", "@railway/cli"])),
        },
        ToolDependency {
            id: "tool.neonctl",
            command: "neonctl",
            required: false,
            reason: "Recommended once Neon CLI adapter is implemented. Neon API can also be used without this CLI.",
            install_command: Some(("npm", &["install", "-g", "neonctl"])),
        },
        ToolDependency {
            id: "tool.stripe",
            command: "stripe",
            required: false,
            reason: "Recommended once Stripe Checkout/webhook adapter is implemented.",
            install_command: None,
        },
    ]
}

fn check_file(
    report: &mut Report,
    root: &Path,
    relative: &'static str,
    id: &'static str,
    required: bool,
) {
    let path = root.join(relative);
    if path.exists() {
        report.findings.push(Finding::ok(
            id,
            "local",
            "present",
            &format!("{relative} exists."),
            &path.display().to_string(),
        ));
    } else if required {
        report.findings.push(Finding::fail(
            id,
            "local",
            "missing",
            &format!("{relative} is missing."),
            "",
            "file_missing",
            &format!("Create {relative} or restore it from Git."),
        ));
    } else {
        report.findings.push(Finding::skipped(
            id,
            "local",
            "missing",
            &format!("{relative} is not present."),
            "optional_file_missing",
        ));
    }
}

fn check_command(report: &mut Report, dependency: ToolDependency) {
    if command_exists(dependency.command) {
        report.findings.push(Finding::ok(
            dependency.id,
            "tool",
            "present",
            &format!("{} is installed.", dependency.command),
            dependency.reason,
        ));
    } else if dependency.required {
        report.findings.push(Finding::fail(
            dependency.id,
            "tool",
            "missing",
            &format!("Required command `{}` is missing.", dependency.command),
            "",
            "tool_missing",
            dependency.reason,
        ));
    } else {
        let repair = match dependency.install_command {
            Some((program, args)) => format!(
                "{} Optional now; install with `{}` or run `cargo xtask external install-deps --yes`.",
                dependency.reason,
                shell_words(program, args)
            ),
            None => dependency.reason.to_string(),
        };
        report.findings.push(Finding::warn(
            dependency.id,
            "tool",
            "missing",
            &format!("Optional command `{}` is missing.", dependency.command),
            &repair,
        ));
    }
}

fn check_gitignore(root: &Path, report: &mut Report) {
    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    for pattern in [
        ".env",
        "setup/.secrets",
        "setup/.state",
        "setup/artifacts",
        "setup/reports",
    ] {
        if gitignore.contains(pattern) {
            report.findings.push(Finding::ok(
                "local.gitignore",
                "local",
                "present",
                &format!(".gitignore covers {pattern}."),
                pattern,
            ));
        } else {
            report.findings.push(Finding::fail(
                "local.gitignore",
                "local",
                "drifted",
                &format!(".gitignore does not appear to cover {pattern}."),
                "",
                "secret_hygiene_gap",
                "Add ignored setup state, report, artifact, and secret paths.",
            ));
        }
    }
}

fn check_env_files(root: &Path, report: &mut Report) {
    let env_paths = [
        (".env", root.join(".env")),
        (".env.local", root.join(".env.local")),
        (
            "setup/.secrets.demo.env",
            root.join("setup/.secrets.demo.env"),
        ),
    ];

    let present: Vec<&str> = env_paths
        .iter()
        .filter_map(|(label, path)| path.exists().then_some(*label))
        .collect();

    if !present.is_empty() {
        report.findings.push(Finding::info(
            "local.env",
            "local",
            "present",
            &format!("Local env files present: {}.", present.join(", ")),
            "Recognized env files are ignored and may contain local demo values.",
        ));
    } else {
        report.findings.push(Finding::warn(
            "local.env",
            "local",
            "missing",
            "No local env file is present.",
            "Copy setup/secrets.example.env to .env or setup/.secrets.demo.env when you are ready to run against provider resources.",
        ));
    }
}

fn check_migrations(root: &Path, report: &mut Report) {
    check_migration_dir(root, report, "migrations", "database.migrations");
    check_migration_dir(
        root,
        report,
        "migrations_postgres",
        "database.migrations.postgres",
    );
}

fn check_migration_dir(
    root: &Path,
    report: &mut Report,
    relative_dir: &'static str,
    id: &'static str,
) {
    let dir = root.join(relative_dir);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) => {
            report.findings.push(Finding::fail(
                id,
                "database",
                "missing",
                &format!("{relative_dir} directory cannot be read."),
                &err.to_string(),
                "migrations_missing",
                &format!("Restore the {relative_dir} directory."),
            ));
            return;
        }
    };

    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".sql"))
        .collect();
    names.sort();

    if names.is_empty() {
        report.findings.push(Finding::fail(
            id,
            "database",
            "missing",
            &format!("No SQL migrations found in {relative_dir}."),
            "",
            "migrations_missing",
            &format!("Add sqlx migration files under {relative_dir}."),
        ));
        return;
    }

    report.findings.push(Finding::ok(
        id,
        "database",
        "present",
        &format!("Found {} SQL migrations in {relative_dir}.", names.len()),
        &names.join(", "),
    ));

    if names.iter().any(|name| name.contains("reviews")) {
        let reviews_id = if relative_dir == "migrations_postgres" {
            "database.migrations.postgres.reviews"
        } else {
            "database.migrations.reviews"
        };
        report.findings.push(Finding::ok(
            reviews_id,
            "database",
            "present",
            &format!("Reviews migration is present in {relative_dir}."),
            "Review schema can be validated once a database adapter is connected.",
        ));
    } else {
        let reviews_id = if relative_dir == "migrations_postgres" {
            "database.migrations.postgres.reviews"
        } else {
            "database.migrations.reviews"
        };
        report.findings.push(Finding::warn(
            reviews_id,
            "database",
            "missing",
            &format!("Reviews migration was not found in {relative_dir}."),
            "Add or restore the reviews migration before enabling review setup.",
        ));
    }
}

fn check_manifest_content(root: &Path, report: &mut Report) {
    let manifest = match fs::read_to_string(root.join("setup/setup.toml")) {
        Ok(manifest) => manifest,
        Err(err) => {
            report.findings.push(Finding::fail(
                "manifest.read",
                "local",
                "missing",
                "Could not read setup/setup.toml.",
                &err.to_string(),
                "manifest_missing",
                "Create setup/setup.toml.",
            ));
            return;
        }
    };

    for required in [
        "[project]",
        "[environment.local]",
        "[[providers]]",
        "[[checks]]",
    ] {
        if manifest.contains(required) {
            report.findings.push(Finding::ok(
                "manifest.section",
                "manifest",
                "present",
                &format!("Manifest includes {required}."),
                required,
            ));
        } else {
            report.findings.push(Finding::fail(
                "manifest.section",
                "manifest",
                "missing",
                &format!("Manifest is missing {required}."),
                "",
                "manifest_incomplete",
                "Update setup/setup.toml to include the required external-world section.",
            ));
        }
    }

    for provider in ["local", "github", "neon", "railway", "stripe"] {
        if manifest.contains(&format!("id = \"{provider}\"")) {
            report.findings.push(Finding::ok(
                "manifest.provider",
                "manifest",
                "present",
                &format!("Manifest declares provider `{provider}`."),
                provider,
            ));
        } else {
            report.findings.push(Finding::warn(
                "manifest.provider",
                "manifest",
                "missing",
                &format!("Manifest does not declare provider `{provider}`."),
                "Add the provider before implementing its adapter.",
            ));
        }
    }
}

fn check_git_tracking(root: &Path, report: &mut Report) {
    let tracked_env = Command::new("git")
        .args([
            "ls-files",
            ".env",
            ".env.local",
            "setup/.secrets.demo.env",
            "setup/.secrets.local.env",
        ])
        .current_dir(root)
        .output();

    match tracked_env {
        Ok(output) if output.status.success() && output.stdout.is_empty() => {
            report.findings.push(Finding::ok(
                "secrets.git_tracking",
                "git",
                "clean",
                "Known local secret files are not tracked by Git.",
                "Secret hygiene check passed.",
            ));
        }
        Ok(output) if output.status.success() => {
            report.findings.push(Finding::fail(
                "secrets.git_tracking",
                "git",
                "invalid",
                "One or more local secret files appear to be tracked by Git.",
                &String::from_utf8_lossy(&output.stdout),
                "secret_tracked",
                "Remove tracked local secret files from Git and rotate any exposed values.",
            ));
        }
        Ok(output) => {
            report.findings.push(Finding::warn(
                "secrets.git_tracking",
                "git",
                "unknown",
                "Could not inspect tracked env files.",
                &String::from_utf8_lossy(&output.stderr),
            ));
        }
        Err(err) => report.findings.push(Finding::warn(
            "secrets.git_tracking",
            "git",
            "unknown",
            "Could not run git tracking check.",
            &err.to_string(),
        )),
    }
}

fn check_env_contract(root: &Path, env_store: &EnvStore, report: &mut Report) {
    for required in required_env_contracts(root) {
        if let Some(value) = env_store.get(&required.name) {
            report.findings.push(Finding::ok(
                "env.runtime",
                "env",
                "present",
                &format!("{} is present in recognized env sources.", required.name),
                &format!("Value redacted. Source: {}.", value.source.label()),
            ));
        } else if required.has_local_fallback {
            report.findings.push(Finding::warn(
                "env.runtime",
                "env",
                "missing_with_fallback",
                &format!(
                    "{} is missing, but setup/setup.toml defines a local fallback.",
                    required.name
                ),
                "Set an explicit value before validating a deployed environment.",
            ));
        } else {
            let local_hint = if root.join(".env").exists()
                || root.join(".env.local").exists()
                || root.join("setup/.secrets.demo.env").exists()
            {
                "A local env file exists, but this value was not found in recognized env sources."
            } else {
                "Copy setup/secrets.example.env to .env or setup/.secrets.demo.env."
            };
            report.findings.push(Finding::fail(
                "env.runtime",
                "env",
                "missing",
                &format!(
                    "{} is required and was not found in recognized env sources.",
                    required.name
                ),
                "",
                "required_env_missing",
                local_hint,
            ));
        }
    }
}

struct RequiredEnv {
    name: String,
    has_local_fallback: bool,
}

fn required_env_contracts(root: &Path) -> Vec<RequiredEnv> {
    let manifest = fs::read_to_string(root.join("setup/setup.toml")).unwrap_or_default();
    let mut required = Vec::new();
    let mut in_required_env = false;
    let mut current_name: Option<String> = None;
    let mut current_has_fallback = false;

    for line in manifest.lines() {
        let line = line.trim();
        if line == "[[required_env]]" {
            if let Some(name) = current_name.take() {
                required.push(RequiredEnv {
                    name,
                    has_local_fallback: current_has_fallback,
                });
            }
            in_required_env = true;
            current_has_fallback = false;
            continue;
        }
        if line.starts_with('[') {
            if in_required_env {
                if let Some(name) = current_name.take() {
                    required.push(RequiredEnv {
                        name,
                        has_local_fallback: current_has_fallback,
                    });
                }
            }
            in_required_env = false;
        }
        if in_required_env && line.starts_with("name") {
            if let Some((_, value)) = line.split_once('=') {
                let value = value.trim().trim_matches('"');
                if !value.is_empty() {
                    current_name = Some(value.to_string());
                }
            }
        } else if in_required_env && line.starts_with("local_fallback") {
            current_has_fallback = true;
        }
    }

    if let Some(name) = current_name.take() {
        required.push(RequiredEnv {
            name,
            has_local_fallback: current_has_fallback,
        });
    }

    if required.is_empty() {
        required.extend(
            [
                "DATABASE_URL",
                "SESSION_SECRET",
                "APP_ENV",
                "PUBLIC_BASE_URL",
            ]
            .iter()
            .map(|name| RequiredEnv {
                name: name.to_string(),
                has_local_fallback: false,
            }),
        );
    }

    required
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

fn repo_root() -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|err| err.to_string())?;
    let mut current = cwd.as_path();
    loop {
        if current.join("Cargo.toml").exists() && current.join("src").exists() {
            return Ok(current.to_path_buf());
        }
        current = current
            .parent()
            .ok_or_else(|| "could not find repo root containing Cargo.toml and src/".to_string())?;
    }
}

fn finish_report(root: &Path, mut report: Report, options: &Options) -> Result<(), String> {
    let finding_count_before_filter = report.findings.len();
    report.retain_only(&options.only);
    if !options.only.is_empty() && finding_count_before_filter > 0 && report.findings.is_empty() {
        report.findings.push(Finding::fail(
            "filter.no_matches",
            "external",
            "empty",
            "The --only selector did not match any findings.",
            &options.only.join(", "),
            "selector_matched_nothing",
            "Use a finding ID, provider name, or prefix shown by an unfiltered report.",
        ));
    }

    if options.write_report {
        write_report_artifact(root, &report)?;
    }

    if options.json {
        println!("{}", render_json_report(&report));
    } else {
        println!("{}", render_human_report(&report));
    }

    exit_if_failed(&report, false)
}

fn write_report_artifact(root: &Path, report: &Report) -> Result<(), String> {
    let reports_dir = root.join("setup/reports");
    fs::create_dir_all(&reports_dir).map_err(|err| {
        format!(
            "failed to create report directory {}: {err}",
            reports_dir.display()
        )
    })?;
    let rendered = render_json_report(report);
    let latest_path = reports_dir.join("latest.json");
    fs::write(&latest_path, &rendered)
        .map_err(|err| format!("failed to write report {}: {err}", latest_path.display()))?;

    let timestamped_path = reports_dir.join(format!(
        "{}-{}.json",
        report.command.replace('.', "-"),
        unix_timestamp()
    ));
    fs::write(&timestamped_path, rendered).map_err(|err| {
        format!(
            "failed to write report {}: {err}",
            timestamped_path.display()
        )
    })
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn exit_if_failed(report: &Report, allow_fail: bool) -> Result<(), String> {
    if report.ok() || allow_fail {
        Ok(())
    } else {
        Err(format!(
            "{} reported failures; rerun with --json for machine-readable output",
            report.command
        ))
    }
}

fn shell_words(program: &str, args: &[&str]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program.to_string());
    parts.extend(args.iter().map(|arg| arg.to_string()));
    parts.join(" ")
}

fn print_help() {
    println!(
        "Davis's Books xtask\n\nCommands:\n  cargo xtask external doctor\n  cargo xtask external plan [--local-only] [--json] [--only <selector>] [--write-report]\n  cargo xtask external validate [--local-only] [--json] [--only <selector>] [--write-report]\n  cargo xtask external setup [--install-deps] [--yes] [--json] [--only <selector>] [--write-report]\n  cargo xtask external repair --only <selector>\n  cargo xtask external install-deps [--yes] [--json] [--only <selector>] [--write-report]\n  cargo xtask external secrets import-email --from <path> [--yes]\n"
    );
}

fn print_external_help() {
    println!(
        "External world commands:\n  doctor        Read-only local bootstrap readiness check\n  plan          Read-only action planning scaffold\n  validate      Read-only local/provider validation report\n  setup         First-slice setup orchestration\n  repair        Targeted repair scaffold; requires --only\n  install-deps  Install supported local dependencies when passed --yes\n  secrets import-email --from <path>\n                Parse a pasted recovery email/note into setup/.secrets.demo.env with --yes\n\nFlags:\n  --json                 Emit machine-readable JSON\n  --local-only           Skip provider checks\n  --only <selector>      Show matching finding IDs/providers only\n  --from <path>          Read a local input file\n  --write-report         Write setup/reports/latest.json\n  --install-deps         Let setup install supported local dependencies\n  --yes, -y              Apply supported installers/writes\n"
    );
}
