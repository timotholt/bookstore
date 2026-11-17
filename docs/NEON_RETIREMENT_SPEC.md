# Neon Retirement Spec

Status: implemented 2026-11-17 (`af62829` code and setup files; docs in the following commit).

## Problem

Production moved from Neon to Railway Postgres on 2026-11-04. The Neon organization is on the Free plan, and the app no longer talks to Neon: the usage monitor was removed in `895eb54` and the branch-testing skill in `ccfd620`.

The external-world bootstrap tool (`cargo xtask external`) still treats Neon as the database provider:

- `xtask/src/providers.rs` (1,101 lines) is mostly a Neon adapter. It can create or verify the Neon project, branch, role and database through the Neon API, fetch a connection URI, and write it into `setup/.secrets.demo.env`. It needs `NEON_API_KEY` and `curl`.
- `xtask/src/main.rs` calls it from `external setup` and `external repair --only neon`, lists `neonctl` as an installable tool, and `validate` warns when `setup/setup.toml` declares no `neon` provider.
- `xtask/src/secrets.rs` lists `NEON_API_KEY` as a known secret for `secrets import-email`.
- `setup/setup.toml` has a `neon` provider block (org id, project `bookstore`, database `neondb`, role `neondb_owner`, branch `production`).
- `setup/secrets.example.env` and `setup/recovery-email.example.txt` (the parser's fake fixture) contain `NEON_API_KEY`.
- `docs/EXTERNAL_WORLD_BOOTSTRAP_SPEC.md` uses Neon as the database provider in its manifest example, `repair` section, sample findings and Phase 2.

Run today, `external setup --yes` would try to create Neon resources and overwrite `DATABASE_URL` in the demo secrets file with a Neon URI. That points away from production and is the wrong default.

## Goals

1. `cargo xtask external` no longer knows about Neon: no API calls, no `NEON_API_KEY`, no `neonctl`, no `neon` manifest block.
2. Keep the provider-neutral checks: `DATABASE_URL` must be `postgres://` or `postgresql://`, and SQLite is rejected.
3. No change to the app binary, migrations, CI or Railway.
4. `doctor`, `plan`, `setup`, `validate` and `repair` still run and report clearly that there is no database provider adapter.

## Non-goals

- **A Railway Postgres adapter.** The production database has no public endpoint and is owned by `postgres`. Migrations go through the operator flow in `PRODUCTION_MIGRATIONS_SPEC.md`, and there is nothing to create through an API. If wanted later, it is its own spec.
- **Rewriting `docs/INFRASTRUCTURE_SPEC.md`.** It still describes "Railway and Neon" throughout and predates the cutover. This work adds a one-line status note at the top that points to the Railway Postgres cutover; a full rewrite is a separate task.
- **Deleting the Neon `bookstore` project.** It is the only pre-cutover copy of the data, and deleting it is an irreversible account action for the owner.

## Design

### xtask

- `providers.rs`: delete every Neon function (`plan_neon_setup`, `apply_neon_setup`, `ensure_neon_*`, `get_neon_connection_uri`, `validate_neon_api`, `validate_neon_branch_collection`, `neon_get_json`, `neon_post_json`) and the helpers only they use (`write_demo_secret`, `shell_quote_env_value`, `find_named_object_id`, `object_identifier`, `object_matches_name_or_id`, `url_encode`, and the local `command_exists`).
- Split `validate_neon_readiness`: drop the `neon.credentials` check and keep the `DATABASE_URL` checks as `validate_database_url_readiness`. Finding ids change from `neon.database_url` / `neon.app_runtime` to `database.url` / `database.app_runtime`, provider `database`. Nothing outside xtask reads these ids.
- `main.rs`:
  - `external setup` drops the Neon step. It still applies migrations with `--yes` when `DATABASE_URL` is present (`repair_database_migrations`, unchanged). Its "not implemented" note lists the database, Railway and Stripe adapters.
  - `external repair --only neon` is removed; an unknown selector falls through to the existing "not implemented" finding.
  - Remove the `tool.neonctl` dependency entry, and drop `neon` from the provider list that `validate` expects in the manifest.
- `secrets.rs`: remove `NEON_API_KEY` from `KNOWN_SECRET_KEYS`. An old recovery email that still contains it is then reported as an unknown key, like any other.

### setup files

- `setup/setup.toml`: delete the `neon` provider block. Change the example `database_url` to `postgresql://user:password@localhost/chantels_corner`.
- `setup/secrets.example.env` and `setup/recovery-email.example.txt`: delete the `NEON_API_KEY` line, and use the same neutral database name in the example URLs.

### Tests

Removed tests, which only covered deleted code: `neon_setup_reports_manifest_state`, `neon_resource_lookup_finds_names_and_ids`, `neon_resource_lookup_handles_numeric_ids`, `url_encode_handles_spaces_and_symbols`, `writes_demo_secret_without_duplicate_keys`.

Kept and renamed (same assertions, new ids):

- `neon_rejects_sqlite_url` becomes `database_url_rejects_sqlite`.
- `neon_reports_postgres_runtime_ready` becomes `database_url_reports_postgres_runtime_ready`.
- `placeholder_values_are_not_usable_credentials` keeps testing `usable_env_value`, using `RAILWAY_TOKEN` instead of `NEON_API_KEY`.
- `manifest.rs` `parses_project_and_provider_desired_state` uses a `railway` block instead of `neon`. It tests the parser, not Neon.
- `env_loader.rs` fixture URL `postgresql://localhost/neondb` becomes `.../chantels_corner`; cosmetic only.

The commit message names every removed test and why.

### Docs

- `EXTERNAL_WORLD_BOOTSTRAP_SPEC.md`: the manifest example drops the `neon` block. Phase 2 (database provider) records that Neon was retired on 2026-11-04 and that no database adapter is planned. The `repair` text and sample findings use the `database.*` ids. Historical narrative stays where it explains past decisions.
- `INFRASTRUCTURE_SPEC.md`: add one status line (see Non-goals).

## Verification

- Full gate: fmt, check, clippy `-D warnings`, `cargo test --workspace` (app and xtask).
- Run each subcommand read-only and check that none mentions Neon or fails because Neon is missing:
  - `cargo xtask external doctor`
  - `cargo xtask external validate --local-only`
  - `cargo xtask external setup --local-only`
- `git grep -i neon -- xtask setup` returns nothing.

## Steps

1. xtask code and tests, setup files (one commit).
2. Docs: the bootstrap spec, the infra status line, and this spec marked implemented (one commit).

Both are repo-only changes, with no production or account actions.
