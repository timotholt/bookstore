# Rust Architecture & Implementation Plan

Davis's Books is now a Rust/Axum storefront. The previous Go server has been retired and removed from the active codebase.

## Canonical Stack

- **Backend:** Rust with Axum.
- **Templates:** Askama, using layout inheritance plus standalone `{% include %}` components.
- **Database:** SQLite locally through `sqlx`, with migrations in `migrations/`.
- **Sessions:** `tower-sessions` memory store for the current local demo.
- **Frontend behavior:** Server-rendered HTML with HTMX fragments for catalog and cart updates.

## Active Entrypoints

- Application entrypoint: `src/main.rs`
- Routes and request handlers: `src/handlers.rs`
- SQL access layer: `src/store.rs`
- View models: `src/models.rs`
- Askama template structs and helpers: `src/templates.rs`
- Schema and seed data: `migrations/20260629000000_schema.sql`

## Template Architecture

The Askama macro-based component approach was retired because macro scoping is brittle in child templates. Components now use standalone include files that read the expected variable from scope.

- `templates/layouts/base.html` is the HTML shell.
- `templates/components/book_cover.html` renders a cover from `book`.
- `templates/components/product_tile.html` renders shelf cards from `book`.
- `templates/components/book_card.html` renders catalog cards from `book`.
- `templates/components/catalog_results.html` renders the HTMX-swappable catalog results.
- `templates/home.html` and `templates/book_detail.html` extend the base layout and include components.

When an include needs a `book` variable, bind or clone the relevant `BookCard` in the parent template scope.

## Local Workflow

```bash
cargo check
cargo run
```

By default the server listens on `http://127.0.0.1:8080` and uses:

```text
DATABASE_URL=sqlite://data/bookstore.db?mode=rwc
```

Override the bind address with:

```bash
ADDR=127.0.0.1:8081 cargo run
```

## Near-Term Roadmap

- Add Rust integration tests for the homepage, catalog fragment, book detail, and cart routes.
- Replace the in-memory session store when production persistence is introduced.
- Add Stripe Checkout handoff and webhook handling in Rust.
- Add staff authentication and CMS inventory workflows in Rust.
- Decide whether production uses PostgreSQL or stays SQLite-backed for the demo deployment.
