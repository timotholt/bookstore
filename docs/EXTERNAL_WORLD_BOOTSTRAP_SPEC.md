# Davis's Books External World Bootstrap Spec

Status: proposed canonical deployment bootstrap and validation spec.

This spec defines how Davis's Books should rebuild, validate, and explain every external dependency required for an interview-ready deployed demo. It extends [INFRASTRUCTURE_SPEC.md](INFRASTRUCTURE_SPEC.md) by treating provider accounts, SaaS configuration, environment variables, database state, auth callbacks, webhooks, and deployment settings as reproducible external state rather than dashboard memory.

## Objective

A clean checkout should be able to guide the owner from repo clone to deployable external world with one command family:

```bash
cargo xtask external doctor
cargo xtask external setup
cargo xtask external validate
```

The system should:

- Discover required local tools, provider accounts, provider CLIs, API tokens, and config files.
- Ask for missing inputs, including pasted demo-secret emails when useful.
- Verify that credentials and accounts are usable before mutating external systems.
- Rebuild external resources idempotently where APIs or CLIs allow it.
- Fall back to documented manual steps or browser automation only when provider APIs are insufficient.
- Run a read-only validation pass that explains what is correct, what failed, and why.
- Produce human-readable and machine-readable reports.
- Leave a durable repo record of what the external world is supposed to look like.

## Core Principle

No external dependency should exist only as dashboard folklore.

For each external resource, the repo must contain at least one of:

- Desired-state manifest.
- Idempotent setup adapter.
- Read-only validation adapter.
- Seed or migration file.
- Manual runbook with exact expected values.
- Browser automation script plus traceable verification.

Dashboard-only setup is acceptable only as an explicit temporary gap, marked `manual` in validation output.

## Non-Goals

- Do not build a general Terraform/Pulumi competitor.
- Do not make local development require paid services.
- Do not require fully automated account creation when providers require email verification, MFA, billing, CAPTCHA, or terms acceptance.
- Do not silently repair production-like infrastructure during validation.
- Do not commit real credentials to Git, even if the demo recovery flow allows plaintext credentials in email or local ignored files.
- Do not use browser automation when a stable API or CLI exists.

## Existing Tooling Landscape

This project should use existing tools where they fit, but it needs a thin project-specific orchestrator because no single tool covers provider accounts, demo-secret ingestion, DB migrations, app smoke checks, auth callbacks, browser fallbacks, and causal validation.

### Terraform / OpenTofu

Terraform and OpenTofu are strong matches for declarative infrastructure resources with provider support. Terraform describes cloud and SaaS resources in versioned configuration, uses providers for APIs, plans changes, and tracks state. Terraform's plan command refreshes remote objects, compares configuration to state, and proposes actions without applying them by default. OpenTofu has the same broad model: write desired state, plan, apply, and maintain state for external resources.

Use for:

- Stable provider resources with mature providers.
- Long-lived infrastructure state.
- Drift detection where the provider ecosystem is good.

Do not use as the only layer because:

- It does not naturally ingest a pasted secrets email.
- It does not own app migrations, app health checks, or demo seed verification.
- Provider coverage for smaller SaaS products may lag.
- It is awkward for dashboard-only actions and guided interactive setup.

### Pulumi

Pulumi is a good fit when infrastructure needs to be expressed in a programming language. Pulumi Automation API is especially relevant because it lets a custom program drive Pulumi operations such as preview, up, refresh, destroy, and stack initialization from application code instead of shelling out directly.

Use for:

- Future richer provider orchestration if the project outgrows simple adapters.
- Programmatic infrastructure workflows that need loops, conditionals, and composition.
- Cases where provider SDKs are better than HCL.

Do not adopt first unless needed. It adds a second infrastructure runtime, project files, state handling, and provider lifecycle. The first Davis's Books bootstrap can be simpler with `xtask` plus provider CLIs/APIs.

### Ansible

Ansible is useful for desired-state automation and emphasizes idempotence: when a target system already matches a playbook, rerunning should make no changes. It is strong for machines, files, packages, and remote system configuration.

Use for:

- Future VM/server configuration if Davis's Books leaves PaaS hosting.
- Human-readable runbooks that execute local/remote commands.

Do not use as the main orchestrator now because this project is mostly SaaS APIs, Rust app verification, and deployment provider state.

### Provider APIs And CLIs

Provider-native APIs and CLIs are the preferred first adapter type.

Examples:

- Railway has a CLI and a GraphQL public API, including project, service, deployment, variable, environment, domain, and volume management.
- Neon exposes an API reference and OpenAPI specification.
- Stripe CLI supports sandbox management, API calls, webhook testing, and integration tasks.
- GitHub can be validated through `git`, `gh`, or the GitHub API.

Use for:

- Account detection.
- Token validation.
- Resource creation/update.
- Environment variables.
- Deployment checks.
- Webhooks where supported.

### Browser Automation

Playwright is a mature headless/headed browser automation tool for modern web apps and supports Chromium, WebKit, and Firefox in local or CI environments.

Use browser automation only as a fallback adapter when:

- A provider has no API/CLI for the required setup.
- The dashboard is the only practical source of truth.
- The automation can store screenshots/traces and validation can independently verify the result.

Browser automation must never be the validation source of truth when an API check exists. It may apply changes; validation should inspect the resulting external state through API, CLI, HTTP, database, or app-level checks whenever possible.

## Recommended Project Decision

Build a Davis's Books `xtask` orchestrator first.

The orchestrator should call provider CLIs/APIs directly and leave room for Terraform/OpenTofu, Pulumi, Ansible, and Playwright as provider adapter implementations. It should not start by adopting a heavyweight IaC engine as the only interface.

Target command shape:

```bash
cargo xtask external doctor
cargo xtask external plan
cargo xtask external setup
cargo xtask external validate
cargo xtask external repair --only database.migrations
cargo xtask external export-report --format json
```

Short aliases may be added later:

```bash
cargo setup-everything
cargo validate-everything
```

## Repository Layout

Target files:

```text
docs/
  EXTERNAL_WORLD_BOOTSTRAP_SPEC.md
  EXTERNAL_WORLD_RUNBOOK.md

setup/
  setup.toml
  environments/
    local.toml
    demo.toml
    production.toml
  providers/
    local.toml
    github.toml
    railway.toml
    neon.toml
    stripe.toml
    auth.toml
  secrets.example.env
  recovery-email.example.txt

xtask/
  Cargo.toml
  src/
    main.rs
    external/
      mod.rs
      manifest.rs
      report.rs
      adapters/
        local.rs
        github.rs
        railway.rs
        neon.rs
        database.rs
        stripe.rs
        auth.rs
        browser.rs
```

Do not create this entire tree before needed. The first implementation should add `xtask`, `setup/setup.toml`, and enough adapters to prove the model.

## Desired-State Manifest

`setup/setup.toml` is the repo-owned summary of expected external state.

Example:

```toml
[project]
name = "davis-books"
owner_email = "timotholt@gmail.com"
github_repo = "timotholt/bookstore"

[environment.demo]
public_base_url = "https://davis-books-demo.up.railway.app"
app_env = "production"

[[providers]]
id = "railway"
adapter = "api"
required = true
account_email = "timotholt@gmail.com"
project = "davis-books"
service = "web"
environment = "production"

[[providers]]
id = "neon"
adapter = "api"
required = true
project = "davis-books"
database = "davis_books"
roles = ["davis_books_app", "davis_books_migrator"]
branches = ["main"]

[[providers]]
id = "stripe"
adapter = "cli"
required = false
mode = "test"
webhook_path = "/webhooks/stripe"

[[checks]]
id = "app.healthz"
kind = "http"
url = "${environment.demo.public_base_url}/healthz"
expect_status = 200

[[checks]]
id = "database.migrations.postgres"
kind = "sqlx_migrations"
provider = "neon"
path = "migrations_postgres"
expect_latest = true
```

The manifest stores identifiers, expected names, expected callback URLs, required env var names, and non-secret desired values. Real secrets belong in local ignored files, provider secret stores, or the owner's recovery email.

## Secret Input Model

The project supports three secret sources:

1. Provider-managed secrets, such as Railway variables or Stripe webhook secrets.
2. Local ignored files, such as `.env.local` or `setup/.secrets.demo.env`.
3. Owner-controlled recovery email or note, optionally plaintext for interview-demo convenience.

Committed files may contain:

- Variable names.
- Example fake values.
- Parsing hints.
- Provider locations.

Committed files must not contain real reusable secrets.

The setup command may support:

```bash
cargo xtask external secrets import-email
cargo xtask external secrets validate
```

The import command should:

- Accept pasted text or a local file path.
- Extract known key/value patterns.
- Show a confirmation diff before writing anything.
- Write only to ignored local files.
- Redact secret values in logs by default.
- Store source metadata such as "parsed from recovery email on YYYY-MM-DD" without storing the original email unless explicitly requested.

## Adapter Contract

Each provider adapter exposes the same lifecycle:

```rust
trait ExternalAdapter {
    fn id(&self) -> &'static str;
    fn detect(&self) -> AdapterDetection;
    fn desired(&self, manifest: &Manifest) -> DesiredState;
    fn inspect(&self, inputs: &Inputs) -> ActualState;
    fn diff(&self, desired: &DesiredState, actual: &ActualState) -> Vec<Finding>;
    fn plan(&self, findings: &[Finding]) -> Vec<Action>;
    fn apply(&self, actions: &[Action], mode: ApplyMode) -> ApplyReport;
    fn validate(&self, desired: &DesiredState, actual: &ActualState) -> ValidationReport;
}
```

Adapter categories:

```text
api       provider HTTP/GraphQL/OpenAPI adapter
cli       provider CLI adapter
iac       Terraform/OpenTofu/Pulumi adapter
browser   Playwright adapter
manual    runbook-only adapter
local     local files/toolchain/process adapter
app       Davis's Books app health/smoke adapter
```

## Command Semantics

### `doctor`

Read-only local readiness check.

Answers:

- Are required local tools installed?
- Is the repo clean enough for setup?
- Are required config files present?
- Are provider CLIs installed?
- Are API tokens present?
- Can local commands run?

It does not contact every external system deeply and does not mutate anything.

### `plan`

Read-only desired-vs-actual planning.

Answers:

- What external resources are missing?
- What resources exist but differ?
- What actions would setup take?
- Which steps require manual or browser fallback?
- Which changes are destructive or risky?

No mutations.

### `setup`

Mutating idempotent apply.

Rules:

- Must run `doctor` first or embed equivalent checks.
- Must verify credentials before applying changes.
- Must print a plan before mutating unless `--yes` is passed.
- Must be idempotent.
- Must not delete or recreate resources by default.
- Must require explicit flags for destructive operations.
- Must write an apply report.
- Must run validation after apply unless `--skip-validate` is passed.

### `validate`

Read-only validation and diagnosis.

Answers:

- Is each dependency configured correctly?
- If not, what failed?
- Is the cause missing credentials, wrong account, missing resource, drift, failed migration, stale callback URL, unreachable endpoint, provider outage, or unknown?
- What command or manual step should fix it?

Validation must not repair. It may suggest:

```bash
cargo xtask external setup --only neon.database
cargo xtask external setup --only railway.variables
cargo xtask external repair --only neon --yes
cargo xtask external repair --only database.migrations
```

### `repair`

Optional targeted mutation for known drift.

Rules:

- Requires `--only`.
- Requires a finding ID or provider path.
- Must show the finding it is repairing.
- Must validate the repaired dependency after apply.

Current implemented repair targets:

- `neon`: idempotently creates or confirms the Neon project, branch, role, and database declared in `setup/setup.toml`, then writes the generated Postgres `DATABASE_URL` to `setup/.secrets.demo.env`.
- `database.migrations`: applies the Postgres migrations in `migrations_postgres/` against the configured `DATABASE_URL`.

## Validation Finding Model

Every finding should contain:

```text
id
provider
resource
severity: ok | info | warn | fail | manual | skipped
status: present | missing | drifted | unreachable | unauthorized | invalid | unknown
summary
evidence
cause
repair
docs
```

Human output example:

```text
[fail] neon.database.reachable
       Could not connect to DATABASE_URL.
       Cause: authentication failed for role davis_books_app.
       Evidence: postgres returned SQLSTATE 28P01.
       Repair: update DATABASE_URL from Neon or run:
               cargo xtask external setup --only neon.connection_strings

[warn] railway.variables
       PUBLIC_BASE_URL is missing on Railway production.
       Cause: required app env var absent.
       Repair: cargo xtask external setup --only railway.variables

[manual] google.oauth
       OAuth consent screen was not inspected.
       Cause: provider has no configured adapter yet.
       Repair: follow docs/EXTERNAL_WORLD_RUNBOOK.md#google-oauth
```

Machine output example:

```json
{
  "ok": false,
  "generated_at": "2026-06-30T00:00:00Z",
  "findings": [
    {
      "id": "neon.database.reachable",
      "provider": "neon",
      "severity": "fail",
      "status": "unauthorized",
      "cause": "authentication_failed",
      "repair_command": "cargo xtask external setup --only neon.connection_strings"
    }
  ]
}
```

## Causal Diagnosis Rules

Validation should report the first actionable cause, not just downstream symptoms.

Examples:

- If `DATABASE_URL` is missing, do not also report every database migration as failed. Mark migration checks as skipped because the database connection prerequisite failed.
- If Railway token is invalid, skip Railway variable and deployment checks with cause `unauthorized`.
- If app `/healthz` fails because the deploy URL is missing, report missing deploy URL first.
- If app `/healthz` returns 500 and database validation also fails, connect the app failure to the database finding when evidence supports it.

This keeps the report useful under interview pressure.

## External Dependency Inventory

Initial dependency graph:

```text
local.toolchain
  -> rust
  -> cargo
  -> sqlx metadata / migrations
  -> optional node/playwright

github.repo
  -> origin URL
  -> default branch
  -> pushed commit

railway.account
  -> railway token or CLI login
  -> project davis-books
  -> service web
  -> environment production
  -> deploy source timotholt/bookstore
  -> runtime variables
  -> public domain

neon.account
  -> neon API key or CLI login
  -> project davis-books
  -> database davis_books
  -> roles
  -> branch main
  -> connection strings
  -> migrations applied

app.runtime
  -> PUBLIC_BASE_URL
  -> DATABASE_URL
  -> SESSION_SECRET
  -> APP_ENV
  -> /healthz
  -> /readyz
  -> /cart
  -> /checkout

stripe.test
  -> test secret key
  -> webhook endpoint
  -> webhook signing secret
  -> checkout success/cancel URLs

auth.email_password
  -> password hashing config
  -> session cookie config
  -> callback URLs only when OAuth ships

reviews
  -> users table
  -> orders table
  -> reviews tables
  -> aggregation checks
```

## Provider Strategy For Davis's Books

### Phase 1: Local Orchestrator Skeleton

Implement:

- `xtask` crate.
- `setup/setup.toml`.
- `cargo xtask external doctor`.
- `cargo xtask external validate --local-only`.
- Report model with human and JSON output.
- Local adapters for repo, env files, Rust toolchain, migrations folder, and app routes.

No external mutations yet.

### Phase 2: Database Provider

Implement:

- Neon desired-state entries.
- Token detection.
- Account/project inspect via API or CLI.
- Database URL validation.
- Migration validation.
- Seed data validation.

Setup may initially print manual Neon steps while validation is real.

### Phase 3: Railway Provider

Implement:

- Railway token detection.
- Project/service/environment inspect.
- Variable validation.
- Public URL validation.
- Healthcheck validation.
- Optional setup for variables and project linking via Railway API/CLI.

### Phase 4: Stripe Test Provider

Implement:

- Stripe CLI/API detection.
- Test key validation.
- Webhook endpoint desired state.
- Webhook signing secret ingestion.
- Checkout smoke in test mode once checkout exists.

### Phase 5: Auth And OAuth Provider

Implement only after email/password auth exists:

- Auth required variables.
- Password pepper/session secret validation.
- OAuth callback desired state.
- Provider-specific adapters when Google OAuth is added.
- Browser/manual fallback if OAuth app APIs are impractical.

## Browser Adapter Policy

Browser setup scripts must:

- Use Playwright.
- Run headed by default for provider dashboards unless `--headless` is explicitly passed.
- Pause for user login, MFA, CAPTCHA, billing acceptance, or terms acceptance.
- Never store provider passwords.
- Store sanitized traces/screenshots under ignored paths.
- Emit a structured result that validation can inspect.
- Prefer stable selectors and URL checks.
- Be considered an apply mechanism, not final proof.

Example:

```bash
cargo xtask external setup --only google.oauth --adapter browser
```

Expected result:

```text
[manual-login] Opened Google Cloud Console.
[apply] Added callback URL.
[validate] Callback URL verified through provider API: ok
```

If provider API validation is unavailable, mark the finding:

```text
[manual] google.oauth.callback
       Browser script completed, but no API validation exists.
       Evidence: screenshot path setup/artifacts/google-oauth-...png
```

## Idempotency Requirements

Setup adapters must be safe to rerun.

Rules:

- Create only if missing.
- Update only if drifted and non-destructive.
- Never rotate secrets unless explicitly requested.
- Never delete provider resources unless `--destroy` or a provider-specific destructive flag is passed.
- Do not create duplicate resources when names already exist.
- Prefer provider IDs stored in ignored local state or discovered by name from the provider.
- If name discovery is ambiguous, stop and ask.

## State Files And Artifacts

Committed:

```text
setup/setup.toml
setup/secrets.example.env
docs/EXTERNAL_WORLD_BOOTSTRAP_SPEC.md
docs/EXTERNAL_WORLD_RUNBOOK.md
```

Ignored:

```text
setup/.state/
setup/.secrets*.env
setup/artifacts/
setup/reports/
```

Reports may be committed only if they contain no secrets and are useful as examples.

## Security Tradeoff For Demos

The project acknowledges the owner's stated demo reality: secrets may live in plaintext in a private email or local personal recovery packet for interview convenience.

The repository still must not commit real secrets. This keeps the public Git history clean while allowing practical local bootstrap:

```text
private email / local ignored file -> parser -> setup/.secrets.demo.env -> provider setup
```

Validation should warn if secrets appear committed or if `.env` files are not ignored.

## CI And Interview Story

CI should eventually run:

```bash
cargo xtask external validate --local-only --json
cargo check
cargo test
```

For an interview demo, the story should be:

1. Clone repo.
2. Install toolchain.
3. Paste demo recovery email or provide local `.env`.
4. Run `cargo xtask external doctor`.
5. Run `cargo xtask external setup`.
6. Run `cargo xtask external validate`.
7. Run app smoke checks.

The report should be legible enough to show:

- What external systems exist.
- Which are automated.
- Which are manual.
- Which failed and why.
- How to repair them.

## Tool Recommendation Summary

Use this stack first:

```text
Rust xtask orchestrator
Provider APIs and CLIs
sqlx migrations
Playwright fallback for dashboard-only setup
Human runbooks for MFA/billing/CAPTCHA-only steps
JSON validation reports for CI/future agents
```

Defer:

```text
OpenTofu/Terraform modules until provider coverage and state needs justify them.
Pulumi Automation API until orchestration becomes too complex for xtask adapters.
Ansible unless the app moves to VM/server configuration.
```

## References

- [Terraform `plan`](https://developer.hashicorp.com/terraform/cli/commands/plan): reads remote objects, compares current configuration to prior state, proposes change actions, supports refresh-only mode, and can emit machine-readable JSON.
- [OpenTofu intro](https://opentofu.org/docs/intro/): open-source infrastructure-as-code workflow with provider-backed desired state.
- [Pulumi Automation API](https://www.pulumi.com/docs/iac/concepts/automation-api/): programmatic interface for running Pulumi programs and building custom CLIs/workflows.
- [Ansible playbooks](https://docs.ansible.com/projects/ansible/latest/playbook_guide/playbooks_intro.html): desired-state automation with idempotent modules and check mode.
- [Railway public API](https://docs.railway.com/integrations/api): GraphQL API for CI/CD and workflow integration, with token-based access and resource-management examples.
- [Railway CLI](https://docs.railway.com/cli): command-line access to deploys, projects, services, variables, environments, domains, and related resources.
- [Neon API Reference](https://api-docs.neon.tech/reference/getting-started-with-neon-api): Neon API and OpenAPI entrypoint.
- [Neon CLI](https://neon.com/docs/reference/neon-cli): command-line project, branch, database, role, operation, and connection-string management.
- [Stripe CLI](https://docs.stripe.com/stripe-cli/use-cli): command-line resource management, webhook forwarding, request logs, and sandbox event triggering.
- [Playwright](https://playwright.dev/docs/intro): browser automation for Chromium, WebKit, and Firefox; useful as a fallback for dashboard-only workflows.
