# Davis's Books

Server-rendered Rust/Axum storefront for Davis's Books, backed locally by SQLite.

The live application entrypoint is `src/main.rs`. The old Go server has been retired; Rust is the only supported backend path. The original static HTML/JS prototype remains archived in `legacy-demo/` for visual reference only and is not served by the Rust app.

## Run Locally

```bash
cargo run
```

The app listens on `http://127.0.0.1:8080` by default and creates `data/bookstore.db` automatically.

Optional environment variables:

```bash
ADDR=127.0.0.1:8081
DATABASE_URL='sqlite://data/bookstore.db?mode=rwc'
APP_ENV=production
```

## Test

```bash
cargo check
```

## Current MVP

- Rust `axum` router and Askama server-rendered templates.
- Local SQLite schema and seed catalog through `sqlx` migrations.
- Server-rendered homepage shelves and catalog cards.
- Include-based Askama component templates for covers, product tiles, catalog cards, and catalog results.
- HTMX catalog search/filter fragments.
- Session-backed cart drawer with quantity updates, stock caps, shipping math, and checkout placeholder.
- Local vendored HTMX runtime at `/assets/htmx.min.js`.
- Archived pre-migration demo under `legacy-demo/` for visual reference.

## Next Production Integrations

- Replace checkout placeholder with Stripe Checkout session creation and webhook handling.
- Add staff auth and CMS inventory screens.
- Decide whether production persistence should use PostgreSQL or SQLite for the hosted demo.
- Move session persistence out of memory before production deployment.

## Architecture Plan

See [MIGRATION_PLAN.md](MIGRATION_PLAN.md) for the Rust architecture notes and remaining roadmap.

## Infrastructure Plan

See [docs/INFRASTRUCTURE_SPEC.md](docs/INFRASTRUCTURE_SPEC.md) for deployment, account ownership, secrets recovery, and production migration planning.
