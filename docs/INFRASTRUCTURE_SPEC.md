# Chantel's Corner Infrastructure, Accounts, and Secret Recovery Spec

Status: describes the running production setup, verified 2026-11-17. Production has run entirely on Railway (web and Postgres) since the Neon → Railway Postgres cutover on 2026-11-04. Neon is retired ([NEON_RETIREMENT_SPEC.md](NEON_RETIREMENT_SPEC.md)).

Related specs:

- Product architecture, feature order, styling and code organization: [PRODUCT_ARCHITECTURE_SPEC.md](PRODUCT_ARCHITECTURE_SPEC.md)
- Schema changes in production: [PRODUCTION_MIGRATIONS_SPEC.md](PRODUCTION_MIGRATIONS_SPEC.md)
- Read budget, catalog cache and usage counters: [DATABASE_READ_BUDGET_AND_CACHE_SPEC.md](DATABASE_READ_BUDGET_AND_CACHE_SPEC.md)
- Transactional email: [RESEND_SPEC.md](RESEND_SPEC.md), [EMAIL_OPERATIONS.md](EMAIL_OPERATIONS.md)
- Staff access bootstrap and recovery: [ADMIN_ACCESS_SPEC.md](ADMIN_ACCESS_SPEC.md)
- `cargo xtask external` setup and validation tooling: [EXTERNAL_WORLD_BOOTSTRAP_SPEC.md](EXTERNAL_WORLD_BOOTSTRAP_SPEC.md)
- Local development and deploy commands: [DEVELOPMENT.md](DEVELOPMENT.md)

## Purpose

Chantel's Corner is a portfolio storefront, not real commerce. It runs as a Rust/Axum binary against PostgreSQL. This spec records where it runs, how its secrets are held, and how to rebuild it if Railway or an account is lost.

## Operating principles

1. The repository holds the whole system shape: code, migrations, Dockerfile, CI, example env files and runbooks. It never holds plaintext secrets: passwords, API keys, connection strings, tokens or signing keys.
2. Runtime secrets live only where they are used (Railway service variables, the owner's ignored local `.env`), plus the owner's recovery packet.
3. The app is Postgres-only and never falls back to another database.
4. Least privilege in production: the web service cannot change the schema. The database owner credential is never stored on the web service.
5. The database has no public endpoint.
6. Rebuilding from the repo and the recovery packet must be possible without anything that exists only in one person's memory.

## Topology

Railway project **Chantel's Corner**, environment `production`:

```text
GitHub timotholt/bookstore (main)
   │  push → GitHub Actions CI → Railway "Wait for CI"
   ▼
web service ── Dockerfile build ── https://www.chantelscorner.com
   │  private network (postgres.railway.internal:5432)
   ▼
Postgres service ── volume /var/lib/postgresql/data
   │  pgBackRest WAL archive
   ▼
Railway storage bucket
```

External services: GitHub (source and CI), Resend (email), and the DNS provider for `chantelscorner.com`.

### web service

- Built from the repository `Dockerfile`, which packages the release binary and the static assets it serves.
- Deploys automatically from `main` once CI passes (Railway "Wait for CI"). CI is `.github/workflows/ci.yml`: fmt, check, clippy, build, tests against a temporary Postgres, and a Docker build.
- One replica. It listens on `0.0.0.0:$PORT` when `ADDR` is unset.
- The healthcheck path is `/readyz`. A build that fails it never receives traffic, and the previous build keeps serving.
- Startup runs with `DATABASE_MIGRATIONS=verify`: it checks the SQLx ledger read-only and exits with a clear error if any migration is pending, failed or mismatched.

### Postgres service

| Item | Value |
| --- | --- |
| Engine | PostgreSQL 18.6 (Railway Postgres image) |
| Volume | `/var/lib/postgresql/data`, 4.6 GB; about 300 MB used on 2026-11-17 |
| App database | `chantels_corner_demo` (79 MB, 37 tables). `postgres` and `railway` also exist and are unused by the app. |
| Network | Private only: `postgres.railway.internal:5432`. No TCP proxy; do not add one. |
| Backups | pgBackRest pushes WAL continuously to a Railway S3 bucket (`WAL_ARCHIVE_*` variables), keeping 4 full and 14 differential backups. On 2026-11-17 it had archived 19,001 segments with no failures. |

Roles:

| Role | Superuser | Purpose |
| --- | --- | --- |
| `postgres` | yes | Owns every object. Applies migrations through `chantels-corner --migrate` (operator only). |
| `chantels_storefront` | no | The web service's `DATABASE_URL`. Row DML on 24 tables; `SELECT` on catalog tables and `_sqlx_migrations`; `CREATE` on the database and the `public` schema for `tower_sessions`. |

Default privileges give `chantels_storefront` `SELECT` on new tables and `USAGE, SELECT` on new sequences created by `postgres`. Write access to a new table is granted explicitly in its migration.

Operator access, never through a public endpoint:

- `railway ssh --service Postgres -- psql -U postgres -d chantels_corner_demo`, for inspection and one-off grants.
- An SSH tunnel through Railway's gateway for `--migrate`, as described in `DEVELOPMENT.md`.

Inside the container, connections from localhost (including tunnelled ones) are trusted without a password. **Any SSH key registered with the Railway account therefore has database superuser access. Treat registered Railway SSH keys as database-owner credentials.**

## Environment variables

Web service variables the app reads. Railway also injects its own `RAILWAY_*` variables; `/version` uses `RAILWAY_GIT_*` and `RAILWAY_DEPLOYMENT_ID`.

| Variable | Secret | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | yes | `chantels_storefront` connection over the private network |
| `APP_ENV` | no | `production` turns on `Secure` session cookies |
| `DATABASE_MIGRATIONS` | no | `verify` in production; unset (`apply`) locally and in CI |
| `PUBLIC_BASE_URL` | no | Absolute links in email and account flows |
| `OPERATIONS_METRICS_TOKEN` | yes | Bearer token for `GET /ops/usage` (32+ characters) |
| `EMAIL_PROVIDER`, `EMAIL_FROM`, `EMAIL_SENDS_PER_DAY` | no | Email delivery settings ([RESEND_SPEC.md](RESEND_SPEC.md)) |
| `EMAIL_OUTBOX_KEY`, `EMAIL_OUTBOX_KEY_ID` | yes | Encryption key for queued email content |
| `RESEND_API_KEY`, `RESEND_WEBHOOK_SECRET` | yes | Resend API and webhook verification |

Operator-only, never set on any Railway service: `MIGRATION_DATABASE_URL`, the owner connection used by `chantels-corner --migrate`.

Optional locally: `ADDR`, `EMAIL_REPLY_TO`, `EMAIL_CAPTURE_DIR`, `EMAIL_TEST_DATABASE_URL`. `setup/secrets.example.env` lists example names with placeholder values.

The Postgres service's own variables (`POSTGRES_*`, `PG*`, `WAL_ARCHIVE_*`) are managed by Railway's template. The web service does not reference them.

## Secret custody

Secrets include every value marked secret above, the `postgres` password, Railway account tokens, registered Railway SSH private keys, Resend account access, and any future Stripe or OAuth credentials.

Allowed homes:

- Railway service variables, only on the service that uses the value.
- The owner's password manager.
- The ignored local `.env` (development values only).
- The owner's recovery packet, kept outside the repository.

Never:

- Anything committed to Git: code, docs, fixtures, migrations or scripts.
- Issue or PR text, CI logs, or chat transcripts.
- The owner database credential stored in Railway. This is why there is no pre-deploy migration command.

## Recovery packet

Kept by the owner outside the repository. It should list:

- The accounts and their login method: GitHub, Railway, Resend, and the DNS registrar for `chantelscorner.com`.
- Railway project, environment and service names and IDs. They appear in `railway status` and on the dashboard.
- The database name, the role names, and where the `postgres` and `chantels_storefront` passwords are kept.
- Which SSH keys are registered with Railway (`railway ssh keys list`).
- The name of every environment variable above, and where each secret value lives.
- The latest applied migration version (`SELECT max(version) FROM _sqlx_migrations`).
- The DNS records for the custom domain.
- The date it was last verified.

## Rebuild from scratch

Use this if the Railway project is lost.

1. Create a Railway project with an environment named `production`. Add a Postgres service from Railway's template, and keep its TCP proxy off.
2. If the old database or its backups survive, restore from them. pgBackRest restore is not yet tested; see Open items. Otherwise start empty.
3. As `postgres`, create `chantels_corner_demo` and the `chantels_storefront` login role. Grant `CONNECT`, plus `CREATE` on the database and the `public` schema, and set up the default privileges listed above.
4. From the Mac, through the SSH tunnel, run `chantels-corner --migrate`. The migrations rebuild the full schema and demo catalog. Then grant `chantels_storefront` its DML on the tables it writes; a live database's `\dp` output, or the recovery packet, is the reference.
5. Add a `web` service connected to `timotholt/bookstore` `main`. Turn on "Wait for CI", set the healthcheck to `/readyz`, and set the variables listed above.
6. Generate a Railway domain. Once it serves, point `www.chantelscorner.com` at it.
7. Run the smoke checks below, then update the recovery packet.

## Health and smoke checks

- `GET /healthz`: the process is up.
- `GET /readyz`: PostgreSQL answers a query. This is the Railway healthcheck.
- `GET /version`: build commit, deployment ID and build time. A deploy is live only when this shows the pushed commit.

After a deploy, check `/`, `/search`, a book detail page, `/catalog` with `HX-Request: true`, `/cart`, `/styles.css` and `/assets/htmx.min.js`. The commands are in [DEVELOPMENT.md](DEVELOPMENT.md).

## Cost

Railway bills usage across both services. The app does not track spend; the Neon spending monitor was removed on 2026-11-17. Check the Railway usage page, and set a usage limit or alert there. Neon is on the Free plan; its last charge is a $0.16 invoice due 2026-12-01.

## Portability

- **Leaving Railway:** the app needs only a Linux container, `PORT` or `ADDR`, and the environment variables above. It has no Railway-specific code; `RAILWAY_*` variables only feed `/version`.
- **Leaving Railway Postgres:** any PostgreSQL 16+ works. Migrations use standard SQL. Recreate the two-role model, then apply migrations as the owner.
- **Email:** delivery sits behind `EMAIL_PROVIDER`; see [RESEND_SPEC.md](RESEND_SPEC.md).
- **Payments:** none. Checkout is a preview and takes no payment. Adding Stripe or similar needs its own spec.

## Open items

- **Backup restore is untested.** WAL archiving works, but nobody has restored from it. Do one restore drill into a scratch service and record the steps here.
- **Railway spend has no alert.** Set a usage limit or alert in Railway.
- **The Neon `bookstore` project** holds the only pre-cutover copy of the data. Delete it once the owner decides it is no longer needed.
- **The recovery packet** has not been checked against this spec since the cutover.

## History

- Before 2026-11-04: Railway web plus Neon Postgres (project `bookstore`, database `neondb`). The earlier version of this document was a target design for that setup. It also covered product roadmap items (cache, sessions, staff CMS, Stripe), which now live in the specs listed at the top.
- 2026-11-04: moved to Railway Postgres with the two-role least-privilege model.
- 2026-11-17: production switched to verify-only startup migrations with an owner-run `--migrate`. The Neon usage monitor and the xtask Neon adapter were removed.
