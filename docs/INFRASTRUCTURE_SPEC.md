# Davis's Books Infrastructure, Accounts, and Secret Recovery Spec

Status: active infrastructure spec. Product architecture, feature order, styling rules, and code organization standards live in [PRODUCT_ARCHITECTURE_SPEC.md](PRODUCT_ARCHITECTURE_SPEC.md).

## Purpose

This document defines how Davis's Books should be deployed as a credible full-stack demo project on Railway and Neon while staying portable enough to rebuild quickly if either provider becomes unusable.

The goal is not PCI-grade production commerce. The goal is a professional interview-ready stack that demonstrates real backend engineering: Rust/Axum server rendering, PostgreSQL persistence, migrations, sessions, Stripe Checkout handoff, staff auth, CMS inventory workflows, and a repeatable infrastructure story.

The external setup and validation automation model is defined in [EXTERNAL_WORLD_BOOTSTRAP_SPEC.md](EXTERNAL_WORLD_BOOTSTRAP_SPEC.md). That spec is the source of truth for `cargo xtask external doctor/setup/validate`, provider adapter policy, validation reports, and reproducible third-party configuration.

## Operating Principles

1. The repository must contain the full system shape: application code, migrations, examples, bootstrap scripts, smoke tests, and runbooks.
2. The repository must never contain plaintext passwords, API keys, connection strings, session keys, Stripe secrets, or provider tokens.
3. Runtime secrets live in the systems that need them: Railway environment variables, Neon project credentials, Stripe webhook secrets, and local `.env.local` files.
4. Account ownership belongs to `timotholt@gmail.com`.
5. Recovery material must be simple enough to use under pressure. If Railway or Neon needs to be replaced, the repo plus the recovery packet should be enough to rebuild.
6. The demo should be reproducible in under 10 minutes after accounts and CLI authentication already exist.
7. The local developer workflow must remain easy. Postgres is the production target, but SQLite can remain as a zero-config local fallback until the PostgreSQL migration is complete.

## Target Providers

### Railway

Railway hosts the Rust/Axum web process.

Responsibilities:

- Build and run the Rust application.
- Store application runtime environment variables.
- Provide public HTTPS routing.
- Run one web service for the Davis's Books storefront and admin panel.
- Optionally run release/migration commands before deploys.

Expected Railway project resources:

- Project: `davis-books`
- Service: `web`
- Environment: `production`
- Deploy source: GitHub repository `timotholt/bookstore`
- Start command: the compiled Rust binary, or Railway's detected Cargo start command if compatible
- Health endpoint: `GET /healthz`

### Neon

Neon hosts PostgreSQL.

Responsibilities:

- Store catalog, copies, carts, orders, CMS users, sessions, and cache metadata.
- Provide pooled and direct database URLs.
- Support migration execution from local CLI or Railway release command.

Expected Neon project resources:

- Project: `davis-books`
- Database: `davis_books`
- Roles:
  - `davis_books_app`: application role with CRUD permissions
  - `davis_books_migrator`: migration role with schema permissions
- Branches:
  - `main`: production-like demo data
  - optional `dev`: staging or local testing branch

## Account Creation Policy

All provider accounts should be created under `timotholt@gmail.com`.

Accounts to create or verify:

- GitHub account with access to `timotholt/bookstore`
- Railway account
- Neon account
- Stripe account or Stripe test-mode project
- Optional domain/DNS provider if a custom domain is later added

Account setup should be performed interactively by the account owner because it may require email verification, OAuth consent, payment method confirmation, or two-factor authentication.

Codex may assist by preparing scripts, provider-specific runbooks, environment variable manifests, and CLI commands. Codex should not store provider passwords in the repository or chat transcript.

## Secret Custody Model

### What Counts as a Secret

Secrets include:

- Railway tokens
- Neon connection strings and passwords
- `DATABASE_URL`
- `SESSION_SECRET`
- Stripe secret keys
- Stripe webhook signing secrets
- Admin bootstrap passwords
- OAuth client secrets
- SMTP credentials
- Any API key for book metadata providers

### Where Secrets May Live

Allowed:

- Railway environment variables
- Neon provider dashboard
- Stripe dashboard
- Local `.env.local`, ignored by Git
- User-owned recovery packet outside the repository
- Email sent by the account owner to themselves, if that is the chosen personal recovery system

Not allowed:

- Plaintext committed to Git
- Plaintext in `README.md`
- Plaintext in migration files
- Plaintext in shell scripts
- Plaintext in test fixtures
- Plaintext in issue descriptions or PR comments

### Recovery Packet

The recovery packet is the user-owned source of truth for rebuilding accounts and infrastructure. It should be prepared after Railway, Neon, and Stripe are configured.

Recommended contents:

- Provider account list
- Login emails
- Project names
- Organization/team names, if any
- Railway project ID
- Railway service ID
- Neon project ID
- Neon database name
- Neon role names
- Stripe account mode and webhook endpoint path
- Environment variable names and where each value lives
- Last known migration version
- GitHub repository URL
- Deployment URL
- Date created and last verified

The recovery packet may include plaintext secrets only if the account owner explicitly chooses that operational tradeoff. The repository must still keep plaintext secrets out.

Recommended safer format:

- `infra/secrets/davis-books.env.example`: committed variable names with fake values
- `infra/secrets/davis-books.recovery.md`: not committed if it contains private account details
- `infra/secrets/davis-books.env.age`: optional encrypted secret bundle if the user chooses local encrypted storage

## Environment Variables

The application should converge on the following environment contract.

Required for production:

```text
APP_ENV=production
PORT=8080
DATABASE_URL=postgres://...
SESSION_SECRET=...
PUBLIC_BASE_URL=https://...
```

Required when Stripe Checkout is enabled:

```text
STRIPE_SECRET_KEY=sk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...
STRIPE_SUCCESS_URL=https://.../checkout/success
STRIPE_CANCEL_URL=https://.../cart
```

Required when staff auth/CMS ships:

```text
ADMIN_BOOTSTRAP_EMAIL=...
ADMIN_BOOTSTRAP_PASSWORD=...
PASSWORD_PEPPER=...
```

Optional:

```text
DATABASE_DRIVER=postgres
SQLITE_DATABASE_URL=file:data/bookstore.db?cache=shared&mode=rwc&_pragma=foreign_keys(1)
LOG_LEVEL=info
BOOK_METADATA_PROVIDER=google_books
GOOGLE_BOOKS_API_KEY=...
```

## Repository Additions Needed

The repo should gain these infrastructure files before a Railway/Neon deploy is considered complete:

```text
infra/
  README.md
  env.example
  railway.md
  neon.md
  secrets/
    README.md
    davis-books.env.example
  scripts/
    check-tools.sh
    bootstrap-neon.sh
    configure-railway.sh
    migrate.sh
    smoke-test.sh
db/
  migrations/
    001_create_catalog.sql
    002_seed_demo_catalog.sql
    003_create_sessions.sql
    004_create_staff_users.sql
```

Scripts should be idempotent where practical. If a script cannot safely create a provider resource automatically, it should print the exact manual command or dashboard step that remains.

## PostgreSQL Migration Requirements

Railway plus Neon requires PostgreSQL as the production database. SQLite can remain for local demos, but production should not depend on SQLite.

Application changes:

- Add a PostgreSQL driver, preferably `pgx`.
- Support `DATABASE_URL` for Postgres.
- Keep SQLite support behind `DATABASE_DRIVER=sqlite` or local fallback.
- Move schema setup out of embedded `db/schema.sql` and into ordered migrations.
- Add a migration runner command or script.
- Replace SQLite-specific SQL syntax where needed.
- Add integration smoke tests against a Postgres database URL.

Schema changes:

- Use `BIGSERIAL` or identity columns where integer IDs are needed.
- Use `BOOLEAN`, `NUMERIC(10,2)`, `TIMESTAMPTZ`, and `TEXT` appropriately.
- Keep cacheable dimensions normalized:
  - authors
  - genres
  - book collections
  - cache tags
  - book/tag joins
- Add indexes for high-traffic pages and filters.

Minimum production indexes:

```sql
CREATE INDEX idx_books_slug ON books(slug);
CREATE INDEX idx_books_primary_author ON books(primary_author_id);
CREATE INDEX idx_books_primary_genre ON books(primary_genre_id);
CREATE INDEX idx_books_search_text ON books(search_text);
CREATE INDEX idx_book_copies_book_available ON book_copies(book_id, is_sold);
CREATE INDEX idx_book_copies_price ON book_copies(price);
CREATE INDEX idx_book_collection_items_collection_position ON book_collection_items(collection_id, position);
CREATE INDEX idx_book_cache_tags_tag ON book_cache_tags(cache_tag_id);
```

Postgres-specific search can later use generated `tsvector` columns or trigram indexes, but the first migration should keep search simple and reliable.

## Cache Strategy

The app should cache repeated public reads while keeping correctness straightforward.

Cacheable page families:

- Homepage shelves
- Product detail page by slug or book ID
- Author page/catalog filter
- Genre page/catalog filter
- Best sellers
- New arrivals
- Used deals
- Staff picks
- Search suggestions

Recommended first implementation:

- In-process Rust cache with short TTLs for public catalog reads.
- Cache keys built from normalized query parameters.
- Cache tags modeled in SQL for future invalidation.
- Explicit invalidation after CMS writes.
- No cache for cart, checkout, admin, auth, or session-specific fragments.

Example cache keys:

```text
home:v1
book:b003:v1
author:frank-herbert:v1
genre:science-fiction:v1
collection:best-sellers:v1
catalog:q=dune:genre=science-fiction:condition=very-good:max=22:sort=popular:v1
```

When the CMS updates a book, the app should invalidate:

- that book detail key
- its author key
- its genre key
- any collection keys containing the book
- homepage key if the book appears in a shelf

For Railway single-service demo traffic, in-process caching is acceptable. If the app later runs multiple replicas, move shared cache state to Redis or accept short TTL inconsistency for public catalog pages.

## Session Strategy

Current session behavior can remain cookie/session-manager based, but production sessions should be database-backed.

Requirements:

- Use `tower-sessions`.
- Set secure cookie flags in production.
- Store session data in Postgres once Postgres is enabled.
- Rotate session tokens on login.
- Do not cache session-specific HTML globally.

Cookie requirements in production:

- `Secure=true`
- `HttpOnly=true`
- `SameSite=Lax`
- short idle lifetime for admin sessions
- longer but reasonable lifetime for anonymous carts

## Auth and CMS Roadmap

The CMS is still missing and should follow the Postgres migration.

Minimum auth/CMS requirements:

- Staff login page.
- Password hashing with bcrypt or Argon2id.
- Admin session guard middleware.
- Initial staff bootstrap command.
- Inventory list.
- Create/edit book metadata.
- Create/edit physical copy records.
- Mark copies sold/unavailable.
- Staff pick flag and quote editor.
- Collection manager for homepage shelves.
- Cache invalidation after writes.

Nice-to-have:

- ISBN lookup through Google Books.
- Image/color generation for placeholder covers.
- Audit log table for admin actions.
- Soft delete for catalog records.

## Stripe Checkout Roadmap

Stripe should be integrated only through Checkout redirection.

Requirements:

- Create Checkout Session from server-side cart.
- Include server-side price and quantity validation before redirect.
- Store pending order before redirect.
- Handle `checkout.session.completed` webhook.
- Mark sold copies or decrement available stock after webhook confirmation.
- Clear cart after successful payment.
- Keep all card handling on Stripe.

For demo mode, Stripe test keys and test checkout are enough.

## Bootstrap Workflow

The target rebuild workflow after accounts and CLIs are authenticated:

1. Clone repo.
2. Install required CLIs if missing:
   - Rust
   - Railway CLI
   - Neon CLI, if used
   - Stripe CLI, optional for webhook testing
3. Copy `infra/env.example` to `.env.local` for local work.
4. Create or select Neon project/database.
5. Run migrations.
6. Create or select Railway project/service.
7. Configure Railway environment variables.
8. Connect Railway service to GitHub repo.
9. Deploy.
10. Run smoke test against public URL.

Target command shape:

```bash
infra/scripts/check-tools.sh
infra/scripts/bootstrap-neon.sh
infra/scripts/configure-railway.sh
infra/scripts/migrate.sh production
infra/scripts/smoke-test.sh https://davis-books.example
```

The exact scripts can evolve, but the final developer experience should be close to this.

## Health and Smoke Checks

The app should expose:

- `GET /healthz`: process is up
- `GET /readyz`: database is reachable and migrations are current

Smoke test assertions:

- Home page returns 200.
- Product detail page returns 200.
- Catalog filter returns 200.
- Add-to-cart HTMX endpoint returns 200.
- Cart drawer shows the added item.
- Checkout placeholder or Stripe redirect route responds correctly.
- Static CSS and HTMX assets return 200.

## Portability Requirements

If Railway is replaced:

- The app must run anywhere that supports a Rust binary and environment variables.
- No Railway-specific code should exist in application packages.
- Railway-specific scripts and docs stay under `infra/`.

If Neon is replaced:

- The app should work with standard PostgreSQL.
- Migrations must avoid Neon-only SQL features unless guarded and documented.
- The recovery packet must include migration version and database role model.

If Stripe is replaced:

- Payment provider code should be isolated behind a small checkout service boundary.
- Orders and carts should remain internal application data.

## Security Boundaries for Demo Use

This project is for portfolio and interview use, not real commerce.

Still required:

- No plaintext secrets in Git.
- HTTPS in deployed environments.
- Secure cookies in production.
- Password hashing for staff accounts.
- Stripe Checkout for payment simulation.
- No raw card storage.
- Minimal admin routes behind auth.

Accepted demo simplifications:

- Stripe test mode.
- Small staff user model.
- In-process public read cache.
- No multi-region deploy.
- No formal PCI compliance process.
- No enterprise secret manager requirement.

## Implementation Phases

### Phase A: Spec and Tooling

- Add this infrastructure spec.
- Add `infra/env.example`.
- Add secrets handling README.
- Add CLI/tool check script.

### Phase B: PostgreSQL Runtime

- Add Postgres driver.
- Add database driver selection.
- Convert schema to ordered migrations.
- Add migration script.
- Verify local app against Postgres.

### Phase C: Railway and Neon Deployment

- Create Neon database.
- Create Railway project/service.
- Configure environment variables.
- Deploy from GitHub.
- Add `/healthz` and `/readyz`.
- Run public smoke test.

### Phase D: Auth and CMS

- Add staff user tables.
- Add login/logout.
- Add protected admin routes.
- Add inventory CRUD.
- Add cache invalidation on writes.

### Phase E: Stripe Checkout

- Add Stripe Checkout session creation.
- Add order tables.
- Add webhook handling.
- Add checkout success/cancel pages.

## Open Decisions

- Whether to keep dual SQLite/Postgres support long term or make Postgres the only supported database after Railway/Neon deployment.
- Whether to keep using embedded `sqlx` migrations, adopt Atlas, or add a small custom migration runner.
- Whether the recovery packet should use plaintext email, encrypted local file, or both.
- Whether Railway deploys should run migrations automatically or require an explicit migration command before deploy.
- Whether the admin CMS should use classic server-rendered forms only or HTMX-enhanced forms.

## Definition of Done

The infrastructure migration is done when:

- The app runs locally with Postgres.
- Railway deploys the app from GitHub.
- Neon stores production demo data.
- Migrations can rebuild the schema from scratch.
- Environment variable names are documented and example values are committed.
- Secrets are absent from Git history.
- The recovery packet exists outside the repo.
- Public smoke tests pass.
- A fresh account/project rebuild can be completed in roughly 10 minutes after CLI authentication.
