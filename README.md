# Chantel's Corner

Chantel's Corner is a portfolio storefront for browsing a seeded collection of new and used books and merchandise. It is a server-rendered Rust application, not a live retail service. Visitors can search and filter the catalog, inspect individual copies, and use a database-backed cart and saved-for-later list. Email/password accounts and profile editing are implemented. Checkout currently **reviews a cart only**: it does not create an order, accept payment, or charge a card.

There is no verified public demo linked here. The supported way to try the application is to run it locally against PostgreSQL.

## What you can demonstrate

- A homepage with catalog shelves, product cards, copy-level pricing and stock, and book detail pages.
- Search and filters rendered on the server, with HTMX replacing catalog results without a full-page reload. `/catalog` serves the HTMX fragment; `/search` is the normal full page.
- Anonymous carts persisted in PostgreSQL, with quantity/stock limits, remove and restore, and saved-for-later actions. Cart state can be recovered from the browser cart cookie after session-store loss.
- Email/password signup and login with Argon2 password hashing; PostgreSQL-backed sessions; account profile and shopping-preference forms.
- A checkout **preview** showing cart lines and totals. The order-history page is an explicit empty state because no order is placed.
- First-party click/search event collection in PostgreSQL, health/readiness endpoints, SQL migrations, and route-level tests.

The catalog and product images are demo data committed with the project. These flows are demonstrable locally, but they are not evidence of real customers, live inventory, or production traffic.

## Stack and structure

| Layer | Implementation |
| --- | --- |
| Web server | Rust 2021, Axum, Tokio |
| UI | Askama templates with reusable includes, CSS, small client-side JavaScript, vendored HTMX |
| Data | PostgreSQL, `sqlx`, ordered migrations and seed catalog |
| Identity | Argon2 password hashes and `tower-sessions` stored in PostgreSQL |
| Operations | `tracing`, `/healthz`, `/readyz`, and an `xtask` for external-dependency checks |

`src/app.rs` defines the routes. `src/handlers.rs` coordinates requests; `src/store.rs`, `src/cart.rs`, and `src/auth.rs` contain data access and domain operations. `src/ui/` prepares reusable view models for the Askama includes under `templates/components/`. `migrations_postgres/` creates and seeds the database. `legacy-demo/` is an archived static prototype, not the running app.

## Build from GitHub

Open [Actions → CI](https://github.com/timotholt/bookstore/actions/workflows/ci.yml), click **Run workflow**, and choose a branch. The run builds and tests the workspace, checks the running Docker image, and provides downloadable image and log artifacts. The manual button is available after the workflow reaches the default branch.

[Complete clean-checkout and one-button build instructions](docs/CLEAN_CHECKOUT.md) cover prerequisites, disposable PostgreSQL, local verification, artifacts, and cleanup.

## Run locally

You need a Rust toolchain with Cargo and a reachable PostgreSQL database. The database role must be able to create tables and the `tower_sessions` schema. This project does **not** support SQLite or silently fall back to another database.

From the repository root, set a real connection string in your shell or in an ignored `.env` / `.env.local` file:

```bash
export DATABASE_URL='postgresql://USER:PASSWORD@HOST:5432/DATABASE'
cargo run --locked
```

`setup/secrets.example.env` lists example variable names; its values are placeholders, not credentials. Never commit a real connection string. On startup the app connects to PostgreSQL, applies pending `migrations_postgres/` migrations (including demo catalog data), initializes its SQL session store, and serves `http://127.0.0.1:8080`. Set `ADDR=127.0.0.1:8081` to choose another local address. Leave `APP_ENV` unset for HTTP localhost; `APP_ENV=production` marks session cookies secure and therefore requires HTTPS for normal browser use.

Try `/`, `/search`, a book linked from the homepage, `/cart`, `/signup`, and `/login`. `/healthz` reports that the web process responds; `/readyz` also queries PostgreSQL. A checkout preview requires a nonempty cart.

## Railway deployment (not live yet)

The repository includes a Dockerfile that builds the Rust binary and packages the static assets it serves from the working directory. Railway supplies `PORT`; when `ADDR` is unset, the app listens on `0.0.0.0:$PORT`. Set the following service variables in Railway:

```text
DATABASE_URL=postgresql://.../...?sslmode=require
APP_ENV=production
```

Use the connection string for the intended Neon database branch. Do not add it to GitHub or the Docker image. Configure Railway's deployment healthcheck path as `/readyz`; it checks that the app can query PostgreSQL. Pending SQLx migrations run **when the web process starts**, before the server listens. Test this against a separate Neon branch before connecting an existing database, because a failed first deploy can still have applied migrations. Do not configure a second pre-deploy migration command for this build.

Connect the GitHub repository and the intended deployment branch in Railway. `.github/workflows/ci.yml` runs format, check, lint, build, tests against temporary PostgreSQL, and a Docker image build on pull requests and pushes to `main`. Enable Railway's **Wait for CI** setting before making `main` an automatic deployment source. Generate a Railway domain only after the service starts successfully, then smoke-test `/`, `/readyz`, `/styles.css`, `/assets/htmx.min.js`, signup/login, catalog, and cart. The Railway project and public demo have not yet been created.

## Validate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --workspace --locked
cargo build --workspace --locked
cargo test --workspace --locked -- --test-threads=1
```

The application route tests require `DATABASE_URL`. They create and drop an isolated PostgreSQL schema, so the test role needs `CREATE` privilege on the database. They run serially because the suite shares a test-schema lock; a remote database can make the full run slow. Formatting, linting, checking, and building do not need a live database.

For a basic HTTP smoke test after startup:

```bash
curl -i http://127.0.0.1:8080/healthz
curl -i http://127.0.0.1:8080/
curl -i http://127.0.0.1:8080/search
curl -i -H 'HX-Request: true' http://127.0.0.1:8080/catalog
curl -i http://127.0.0.1:8080/cart
```

## Not implemented yet

- Stripe payment handoff, order creation, receipts, and real order history.
- Anonymous-cart merge into a user-owned cart at login; current carts and saved items are session/browser keyed, not durable account-owned lists.
- Customer review submission, voting, moderation, and verified-purchase status. Review tables and aggregate reads are groundwork, not a complete review feature.
- Staff authentication and catalog/inventory management UI.
- Google login, email verification, password reset, and a confirmed public deployment.

The [product architecture](docs/PRODUCT_ARCHITECTURE_SPEC.md), [infrastructure plan](docs/INFRASTRUCTURE_SPEC.md), [review design](docs/REVIEWS_SPEC.md), and [external setup design](docs/EXTERNAL_WORLD_BOOTSTRAP_SPEC.md) describe intended work as well as current code; they are not a list of shipped features. [AGENTS.md](AGENTS.md) contains repository engineering guidance.

The Cargo package and binary are named `chantels-corner`, and new carts use the `chantels_cart_key` browser cookie. The original applied migration, archived static prototype, and local repository path retain legacy naming for migration integrity or historical context. These are not the customer-facing brand.
