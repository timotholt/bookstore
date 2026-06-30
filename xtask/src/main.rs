use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
            emit_report(&report, options.json);
            exit_if_failed(&report, false)
        }
        [cmd, flags @ ..] if cmd == "validate" => {
            let options = Options::parse(flags)?;
            let report = build_validation_report(root, options.local_only);
            emit_report(&report, options.json);
            exit_if_failed(&report, false)
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
            emit_report(&report, options.json);
            exit_if_failed(&report, false)
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
            report.findings.push(Finding::manual(
                "setup.providers",
                "external",
                "not_implemented",
                "Provider setup adapters are not implemented in this first slice.",
                "Next adapters should be Neon database validation/setup, then Railway variables/deploy validation.",
            ));

            emit_report(&report, options.json);
            exit_if_failed(&report, false)
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
}

impl Options {
    fn parse(flags: &[String]) -> Result<Self, String> {
        let mut options = Options::default();
        for flag in flags {
            match flag.as_str() {
                "--json" => options.json = true,
                "--yes" | "-y" => options.yes = true,
                "--local-only" => options.local_only = true,
                "--install-deps" => options.install_deps = true,
                "--help" | "-h" => {
                    print_external_help();
                    std::process::exit(0);
                }
                unknown => return Err(format!("unknown flag: {unknown}")),
            }
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
    check_env_contract(root, &mut report);

    if local_only {
        report.findings.push(Finding::info(
            "validate.scope",
            "external",
            "local_only",
            "External provider checks skipped by --local-only.",
            "Run without --local-only after provider adapters are implemented.",
        ));
    } else {
        report.findings.push(Finding::manual(
            "validate.providers",
            "external",
            "not_implemented",
            "Provider inspection adapters are not implemented yet.",
            "The first provider adapter should inspect Neon account/project/database/migration state.",
        ));
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
                    "sqlite,rustls",
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

#[derive(Clone)]
struct Report {
    command: &'static str,
    findings: Vec<Finding>,
}

impl Report {
    fn new(command: &'static str) -> Self {
        Self {
            command,
            findings: Vec::new(),
        }
    }

    fn extend(&mut self, other: Report) {
        self.findings.extend(other.findings);
    }

    fn ok(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Fail)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Severity {
    Ok,
    Info,
    Warn,
    Fail,
    Manual,
    Skipped,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Ok => "ok",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Fail => "fail",
            Severity::Manual => "manual",
            Severity::Skipped => "skipped",
        }
    }
}

#[derive(Clone)]
struct Finding {
    id: &'static str,
    provider: &'static str,
    severity: Severity,
    status: &'static str,
    summary: String,
    evidence: String,
    cause: &'static str,
    repair: String,
}

impl Finding {
    fn ok(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        evidence: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Ok,
            status,
            summary,
            evidence,
            "none",
            "",
        )
    }

    fn info(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        evidence: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Info,
            status,
            summary,
            evidence,
            "none",
            "",
        )
    }

    fn warn(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        repair: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Warn,
            status,
            summary,
            "",
            "attention_required",
            repair,
        )
    }

    fn fail(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        evidence: &str,
        cause: &'static str,
        repair: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Fail,
            status,
            summary,
            evidence,
            cause,
            repair,
        )
    }

    fn manual(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        repair: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Manual,
            status,
            summary,
            "",
            "manual_step_required",
            repair,
        )
    }

    fn skipped(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        cause: &'static str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Skipped,
            status,
            summary,
            "",
            cause,
            "",
        )
    }

    fn new(
        id: &'static str,
        provider: &'static str,
        severity: Severity,
        status: &'static str,
        summary: &str,
        evidence: &str,
        cause: &'static str,
        repair: &str,
    ) -> Self {
        Self {
            id,
            provider,
            severity,
            status,
            summary: summary.to_string(),
            evidence: evidence.to_string(),
            cause,
            repair: repair.to_string(),
        }
    }
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
    if root.join(".env").exists() {
        report.findings.push(Finding::info(
            "local.env",
            "local",
            "present",
            ".env exists locally.",
            ".env is ignored and may contain local demo values.",
        ));
    } else {
        report.findings.push(Finding::warn(
            "local.env",
            "local",
            "missing",
            ".env is not present.",
            "Copy setup/secrets.example.env to .env or setup/.secrets.demo.env when you are ready to run against provider resources.",
        ));
    }
}

fn check_migrations(root: &Path, report: &mut Report) {
    let dir = root.join("migrations");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) => {
            report.findings.push(Finding::fail(
                "database.migrations",
                "database",
                "missing",
                "migrations directory cannot be read.",
                &err.to_string(),
                "migrations_missing",
                "Restore the migrations directory.",
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
            "database.migrations",
            "database",
            "missing",
            "No SQL migrations found.",
            "",
            "migrations_missing",
            "Add sqlx migration files under migrations/.",
        ));
        return;
    }

    report.findings.push(Finding::ok(
        "database.migrations",
        "database",
        "present",
        &format!("Found {} SQL migrations.", names.len()),
        &names.join(", "),
    ));

    if names.iter().any(|name| name.contains("reviews")) {
        report.findings.push(Finding::ok(
            "database.migrations.reviews",
            "database",
            "present",
            "Reviews migration is present.",
            "Review schema can be validated once a database adapter is connected.",
        ));
    } else {
        report.findings.push(Finding::warn(
            "database.migrations.reviews",
            "database",
            "missing",
            "Reviews migration was not found.",
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
        .args(["ls-files", ".env"])
        .current_dir(root)
        .output();

    match tracked_env {
        Ok(output) if output.status.success() && output.stdout.is_empty() => {
            report.findings.push(Finding::ok(
                "secrets.git_tracking",
                "git",
                "clean",
                ".env is not tracked by Git.",
                "Secret hygiene check passed.",
            ));
        }
        Ok(output) if output.status.success() => {
            report.findings.push(Finding::fail(
                "secrets.git_tracking",
                "git",
                "invalid",
                ".env appears to be tracked by Git.",
                &String::from_utf8_lossy(&output.stdout),
                "secret_tracked",
                "Remove .env from Git tracking and rotate any exposed values.",
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

fn check_env_contract(root: &Path, report: &mut Report) {
    for name in [
        "DATABASE_URL",
        "SESSION_SECRET",
        "APP_ENV",
        "PUBLIC_BASE_URL",
    ] {
        if env::var(name).is_ok() {
            report.findings.push(Finding::ok(
                "env.runtime",
                "env",
                "present",
                &format!("{name} is present in the process environment."),
                "Value redacted.",
            ));
        } else {
            let local_hint = if root.join(".env").exists() {
                ".env exists; runtime loading is handled by the app, not xtask."
            } else {
                "Copy setup/secrets.example.env to .env or setup/.secrets.demo.env."
            };
            report.findings.push(Finding::warn(
                "env.runtime",
                "env",
                "missing",
                &format!("{name} is not present in the process environment."),
                local_hint,
            ));
        }
    }
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

fn emit_report(report: &Report, json: bool) {
    if json {
        println!("{}", render_json_report(report));
    } else {
        println!("{}", render_human_report(report));
    }
}

fn render_human_report(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "External World Report: {}", report.command);
    let _ = writeln!(out, "status: {}", if report.ok() { "ok" } else { "failed" });
    let _ = writeln!(out);

    for finding in &report.findings {
        let _ = writeln!(
            out,
            "[{}] {} ({})",
            finding.severity.as_str(),
            finding.id,
            finding.status
        );
        let _ = writeln!(out, "      {}", finding.summary);
        if !finding.evidence.is_empty() {
            let _ = writeln!(out, "      Evidence: {}", finding.evidence.trim());
        }
        if finding.cause != "none" {
            let _ = writeln!(out, "      Cause: {}", finding.cause);
        }
        if !finding.repair.is_empty() {
            let _ = writeln!(out, "      Repair: {}", finding.repair);
        }
    }

    out
}

fn render_json_report(report: &Report) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"command\":\"{}\",\"ok\":{},\"findings\":[",
        json_escape(report.command),
        report.ok()
    );

    for (idx, finding) in report.findings.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"id\":\"{}\",\"provider\":\"{}\",\"severity\":\"{}\",\"status\":\"{}\",\"summary\":\"{}\",\"evidence\":\"{}\",\"cause\":\"{}\",\"repair\":\"{}\"}}",
            json_escape(finding.id),
            json_escape(finding.provider),
            finding.severity.as_str(),
            json_escape(finding.status),
            json_escape(&finding.summary),
            json_escape(&finding.evidence),
            json_escape(finding.cause),
            json_escape(&finding.repair)
        );
    }

    out.push_str("]}");
    out
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

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

fn shell_words(program: &str, args: &[&str]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program.to_string());
    parts.extend(args.iter().map(|arg| arg.to_string()));
    parts.join(" ")
}

fn print_help() {
    println!(
        "Davis's Books xtask\n\nCommands:\n  cargo xtask external doctor\n  cargo xtask external validate [--local-only] [--json]\n  cargo xtask external setup [--install-deps] [--yes] [--json]\n  cargo xtask external install-deps [--yes] [--json]\n"
    );
}

fn print_external_help() {
    println!(
        "External world commands:\n  doctor        Read-only local bootstrap readiness check\n  validate      Read-only local/provider validation report\n  setup         First-slice setup orchestration\n  install-deps  Install supported local dependencies when passed --yes\n\nFlags:\n  --json          Emit machine-readable JSON\n  --local-only    Skip provider checks\n  --install-deps  Let setup install supported local dependencies\n  --yes, -y       Apply supported installers\n"
    );
}
