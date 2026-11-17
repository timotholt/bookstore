# Production Migrations Spec

Status: approved 2026-11-17. Steps 1–2 (code) are implemented; steps 3–4 (production configuration) are pending.

## Problem

Production runs on Railway Postgres under a least-privilege grant model, set up during the 2026-11-04 Neon → Railway cutover:

- `postgres` owns every object in `chantels_corner_demo` (37 tables, 16 sequences, 4 functions, `_sqlx_migrations`).
- The web service connects as `chantels_storefront`: no superuser rights, row-level DML on 24 tables, `SELECT` only on the catalog tables and on `_sqlx_migrations`, and `CREATE` on the database and the `public` schema (needed for `tower_sessions`).
- There are no default privileges (`pg_default_acl` is empty).

The app runs `sqlx::migrate!().run()` at startup, as `chantels_storefront`. That only works while there is nothing to apply. Any new migration fails. `ALTER`/`CREATE INDEX` on an existing table fails with `42501 must be owner`, and even a pure `CREATE TABLE` fails when sqlx writes the ledger row. The container crash-loops, the healthcheck fails, and Railway quietly keeps the previous build serving. On 2026-11-17 this blocked three deploys until the `expires_at` index migration was removed (`71916c8`).

The grant model is deliberate and stays. If the app login is compromised (leaked secret, injection bug), it cannot drop or alter tables or rewrite migration history.

## Goals

1. A new migration reaches production through an explicit owner-run step, never through the app role.
2. A deploy that needs a migration nobody has applied fails loudly with a clear log line, not a bare `42501`.
3. No owner or superuser secret is stored in the web service's environment.
4. Local development and CI are unchanged: there the app role owns the database and keeps applying migrations at startup.

## Design

### 1. App startup: apply or verify

Add `DATABASE_MIGRATIONS=apply|verify` (default `apply`).

- `apply`: today's behaviour. Used locally and in CI.
- `verify`: never writes. It reads `_sqlx_migrations` and compares it with the migrations embedded in the binary. Pending versions, checksum mismatches, or failed rows log one `error` line naming each version and pointing to this spec, and the process exits non-zero. Everything applied and matching → startup continues.

Production sets `DATABASE_MIGRATIONS=verify` on the web service. A test covers each verify outcome: clean, pending, and checksum mismatch.

### 2. Owner-run migration command

Add a binary subcommand, `chantels-corner --migrate`, that applies the embedded migrations using `MIGRATION_DATABASE_URL` and then exits. Because it is the same binary, its checksums are by construction the same as the build being deployed.

Operator flow, run from the Mac. The owner credential lives only in the operator's password manager and shell session:

1. Write the migration so the currently deployed code keeps working after it is applied (additive: new tables, columns with defaults, indexes). Destructive changes are split across two deploys (expand, then contract).
2. Open a private tunnel to the database: `railway ssh config --service Postgres` once, then `ssh -L 127.0.0.1:55440:localhost:5432 <railway-host>`. No public TCP proxy.
3. `MIGRATION_DATABASE_URL=postgres://postgres@127.0.0.1:55440/chantels_corner_demo cargo run --release -- --migrate`, with the password supplied through a `~/.pgpass` entry (mode 600) or `PGPASSWORD` in that shell only. There is no interactive prompt.
4. Push. The new web build starts in `verify` mode, finds the ledger complete, and passes its healthcheck.

The order (migrate first, then push) is why rule 1 matters: for a few minutes the old build runs against the new schema.

### 3. Grants for objects created by migrations

Objects created by `postgres` are not visible to the app unless granted. Two parts:

- One-time operator step: `ALTER DEFAULT PRIVILEGES FOR ROLE postgres IN SCHEMA public GRANT SELECT ON TABLES TO chantels_storefront;` and `... GRANT USAGE, SELECT ON SEQUENCES TO chantels_storefront;`. New tables are then readable by default, which matches how the catalog is treated today.
- Write access stays explicit. A migration that creates a table the app must write to ends with a guarded grant, so it runs locally where the role does not exist:

  ```sql
  DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'chantels_storefront') THEN
      GRANT INSERT, UPDATE, DELETE ON new_table TO chantels_storefront;
    END IF;
  END $$;
  ```

### 4. First use

Re-add the dropped `account_email_rate_limits (expires_at)` index through this flow, as the end-to-end proof.

## Rejected alternatives

- **Give the app role ownership.** It undoes the cutover's hardening, and the app could then drop any table.
- **Railway pre-deploy command with `MIGRATION_DATABASE_URL` on the web service.** Fully automatic, but the owner credential would sit in the web service's environment, readable by the running app process.
- **Separate Railway `migrator` service holding the owner URL.** Keeps the secret off the web service, but it deploys from the same push at the same time as web, so ordering becomes a race. Revisit if migrations become frequent.
- **Applying SQL by hand with `psql`.** Skips the sqlx ledger and checksums, so the next `verify` would fail.

## Implementation steps

1. `DATABASE_MIGRATIONS` verify mode plus tests (code). This alone turns today's silent crash-loop into a clear error.
2. `--migrate` subcommand reading `MIGRATION_DATABASE_URL` (code), and an operator section in `docs/DEVELOPMENT.md`.
3. Production configuration, operator only: set `DATABASE_MIGRATIONS=verify` on the web service and run the one-time default-privileges grant.
4. Re-add the `expires_at` index through the new flow and confirm the deploy goes live (`/version`, `/readyz`).

Steps 1–2 are ordinary repo changes. Steps 3–4 change production and each need an explicit go-ahead.
