# Development and deployment

[Back to the project](../README.md) · [Documentation index](README.md)

## Development setup

You need a Rust toolchain with Cargo and a reachable PostgreSQL database. The database role must be able to create tables and the `tower_sessions` schema. This project does **not** support SQLite or silently fall back to another database.

From the repository root, set a real connection string in the ignored `.env` file:

```bash
export DATABASE_URL='postgresql://USER:PASSWORD@HOST:5432/DATABASE'
cargo run --locked
```

[`setup/secrets.example.env`](../setup/secrets.example.env) lists example variable names; its values are placeholders, not credentials. Never commit a real connection string. On startup the app connects to PostgreSQL, applies pending `migrations_postgres/` migrations (including demo catalog data), initializes its SQL session store, and starts the web server. Use `ADDR` to configure the development bind address and port. Leave `APP_ENV` unset for development over HTTP; `APP_ENV=production` marks session cookies secure and therefore requires HTTPS for normal browser use.

Try `/`, `/search`, a book linked from the homepage, `/cart`, `/signup`, and `/login`. `/healthz` reports that the web process responds; `/readyz` also queries PostgreSQL. A checkout preview requires a nonempty cart.

## Railway deployment

[Live portfolio demo](https://www.chantelscorner.com/) — homepage and database readiness verified on 2026-09-19. Checkout is a preview; no payment is collected.

The repository includes a Dockerfile that builds the Rust binary and packages the static assets it serves from the working directory. Railway supplies `PORT`; when `ADDR` is unset, the app listens on `0.0.0.0:$PORT`. Set the following service variables in Railway:

```text
DATABASE_URL=postgresql://.../...?sslmode=require
APP_ENV=production
```

Use the connection string for the intended Neon database branch. Do not add it to GitHub or the Docker image. Configure Railway's deployment healthcheck path as `/readyz`; it checks that the app can query PostgreSQL. Pending SQLx migrations run **when the web process starts**, before the server listens. Test this against a separate Neon branch before connecting an existing database, because a failed first deploy can still have applied migrations. Do not configure a second pre-deploy migration command for this build.

Connect the GitHub repository and the intended deployment branch in Railway. `.github/workflows/ci.yml` runs format, check, lint, build, tests against temporary PostgreSQL, and a Docker image build on pull requests and pushes to `main`. Enable Railway's **Wait for CI** setting before making `main` an automatic deployment source. Generate a Railway domain only after the service starts successfully, then smoke-test `/`, `/readyz`, `/styles.css`, `/assets/htmx.min.js`, signup/login, catalog, and cart. The public demo is available at the link above.

## Validate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --workspace --locked
cargo build --workspace --locked
cargo test --workspace --locked -- --test-threads=1
```

The application route tests require `DATABASE_URL`. They create and drop an isolated PostgreSQL schema, so the test role needs `CREATE` privilege on the database. They run serially because the suite shares a test-schema lock; a remote database can make the full run slow. Formatting, linting, checking, and building do not need a live database.

For read-only smoke checks against the public HTTPS demo:

```bash
DEMO_BASE_URL=https://www.chantelscorner.com
curl -i "$DEMO_BASE_URL/healthz"
curl -i "$DEMO_BASE_URL/readyz"
curl -i "$DEMO_BASE_URL/"
curl -i "$DEMO_BASE_URL/search"
curl -i -H 'HX-Request: true' "$DEMO_BASE_URL/catalog"
curl -i "$DEMO_BASE_URL/cart"
```


## Repository naming

The Cargo package and binary are `chantels-corner`. The original applied migration and archived static prototype retain legacy names for migration integrity and historical context. New public-facing copy uses Chantel’s Corner.
