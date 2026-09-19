<p align="center">
  <img src="docs/assets/repository-cover.png" alt="Chantel’s Corner — a thoughtfully built bookstore. Rust, Axum, Askama, HTMX, PostgreSQL." width="100%">
</p>

<h1 align="center">Chantel’s Corner</h1>

<p align="center">A bookstore portfolio project with server-rendered pages, responsive catalog interactions, and persistent shopping carts.</p>

<p align="center">
  <a href="https://github.com/timotholt/bookstore/actions/workflows/ci.yml"><img src="https://github.com/timotholt/bookstore/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <img src="https://img.shields.io/badge/Rust-2021-b7410e?style=flat" alt="Rust 2021 edition">
  <img src="https://img.shields.io/badge/UI-Askama%20%2B%20HTMX-17545b?style=flat" alt="Askama and HTMX UI">
  <img src="https://img.shields.io/badge/Database-PostgreSQL-336791?style=flat" alt="PostgreSQL database">
</p>

<p align="center">
  <a href="https://www.chantelscorner.com/">Live demo</a> ·
  <a href="#product-tour">Product tour</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#run-locally">Run locally</a> ·
  <a href="docs/README.md">Documentation</a>
</p>

Browse new and used books, compare individual copies, filter the catalog, and build a reading stack. The application combines a Rust backend with HTML-first interactions and PostgreSQL persistence.

**Portfolio demo:** the catalog is seeded sample data. Checkout reviews the cart; it does not place orders, take payments, or charge a card. Some storefront copy and ratings are illustrative. See [current scope](#current-scope) for implemented features and planned work.

## Product tour

### A bookstore, from the first page

Curated shelves, category browsing, new arrivals, and copy-level prices give the catalog a familiar storefront experience.

![Chantel’s Corner homepage with category browsing, featured books, and curated shelves](docs/assets/homepage.png)

### Search → inspect → add to cart

The walkthrough below was recorded from the running Rust application against an isolated local PostgreSQL database.

![Product walkthrough: filter the catalog, inspect a book, and add it to the cart](docs/assets/shopping-walkthrough.gif)

[Download the walkthrough video](docs/assets/shopping-walkthrough.mp4) · [Screenshot details and capture notes](docs/PRESENTATION.md)

<table>
  <tr>
    <td width="50%"><a href="docs/assets/catalog.png"><img src="docs/assets/catalog.png" alt="Catalog results with genre, condition, price, and format filters"></a></td>
    <td width="50%"><a href="docs/assets/book-detail.png"><img src="docs/assets/book-detail.png" alt="Book detail page showing copy options, condition, price, and availability"></a></td>
  </tr>
  <tr>
    <td><strong>Find the next read.</strong> Search and filter with server-rendered results swapped into the page by HTMX.</td>
    <td><strong>Choose a specific copy.</strong> Inspect format, condition notes, price, and stock before adding it.</td>
  </tr>
  <tr>
    <td><a href="docs/assets/cart.png"><img src="docs/assets/cart.png" alt="Shopping cart with quantity controls, saved items, and an order summary"></a></td>
    <td><a href="docs/assets/account.png"><img src="docs/assets/account.png" alt="Account area for a fictional demo reader"></a></td>
  </tr>
  <tr>
    <td><strong>Keep a reading stack.</strong> Database-backed carts support quantity limits, removal, restore, and saved-for-later actions.</td>
    <td><strong>Manage an account.</strong> Email/password authentication, profile editing, and shopping preferences.</td>
  </tr>
</table>

## Architecture

![Illustrated architecture: browser requests pass through Axum and Rust handlers, SQLx queries PostgreSQL, and Askama returns HTML for HTMX updates; Tokio runs asynchronous work](docs/study-assets/bookstore-stack-professional.png)

**Request path:** browser → Axum handler → domain/data-access code → SQLx ↔ PostgreSQL → prepared view data → Askama HTML → browser.

| Layer | Tools | Responsibility |
| --- | --- | --- |
| Application | Rust, Axum, Tokio | Typed application logic, routing, asynchronous I/O |
| Presentation | Askama, HTMX, HTML, CSS | Server-rendered pages, reusable includes, targeted updates |
| Persistence | PostgreSQL, SQLx | Catalog, carts, accounts, sessions, ordered migrations |
| Identity | Argon2, tower-sessions | Password hashing and PostgreSQL-backed sessions |
| Operations | tracing, GitHub Actions, Docker | Diagnostics, automated checks, portable packaging |

### Engineering decisions

- **HTML-first delivery.** Askama renders both pages and fragments. HTMX enhances interactions without a separate frontend application.
- **Explicit persistent state.** Cart and session records live in PostgreSQL. SQLx migrations define the schema and seed catalog.
- **Reusable UI patterns.** Rust view objects feed Askama include components and shared CSS class families. The [component contract](docs/PRODUCT_ARCHITECTURE_SPEC.md#ui-pattern-system) keeps repeated controls consistent.
- **Focused application modules.** Route handlers coordinate requests; store, cart, and auth modules own data access and domain operations.
- **Observable, reproducible builds.** CI checks formatting, compilation, linting, tests against PostgreSQL, and the Docker build. `/healthz` reports process health; `/readyz` checks database connectivity.

[Read the architecture specification](docs/PRODUCT_ARCHITECTURE_SPEC.md) · [Learn the stack](docs/STACK_STUDY_GUIDE.md)

## Current scope

| Implemented | Planned |
| --- | --- |
| Search, filters, book details, and seeded merchandise | Staff inventory management |
| PostgreSQL-backed carts and saved-for-later actions | Durable account-owned carts and login merge |
| Email/password signup, login, profiles, preferences | Google login, email verification, password reset |
| Cart review and checkout preview | Payment handoff, order creation, receipts |
| Review schema and aggregate reads | Review submission, moderation, verified purchases |
| First-party interaction events, health checks, CI | Additional deployment and operational hardening |

Cart identity is currently tied to the browser/session. Signing in does not yet merge it into a durable user-owned cart. The order-history screen is an explicit empty state because checkout does not create orders.

## Run locally

Requires a Rust toolchain with Cargo and a reachable PostgreSQL database. The database role needs permission to create application tables and the `tower_sessions` schema.

```bash
git clone https://github.com/timotholt/bookstore.git
cd bookstore
export DATABASE_URL='postgresql://USER:PASSWORD@HOST:5432/DATABASE'
cargo run --locked
```

Open **http://127.0.0.1:8080**. Startup applies pending migrations and seeds the demo catalog. Set `ADDR=127.0.0.1:8081` to use another port. Keep credentials in the shell or an ignored local environment file.

[Complete setup, testing, and deployment instructions](docs/DEVELOPMENT.md)

## Explore the repository

```text
src/
  app.rs          Routes and shared application state
  handlers.rs     Request and response coordination
  store.rs        Catalog and data access
  cart.rs         Cart and saved-item operations
  auth.rs         Account and authentication operations
  ui/             Reusable view models
templates/        Askama pages, layouts, and include components
styles.css        Shared styles and design tokens
migrations_postgres/  Ordered database migrations and seed data
.github/workflows/    Continuous integration
docs/             Architecture, development, and product guides
```

[Documentation index](docs/README.md) · [Development guide](docs/DEVELOPMENT.md) · [Infrastructure design](docs/INFRASTRUCTURE_SPEC.md) · [Technical debt](docs/TECH_DEBT.md)

The active application is Rust and PostgreSQL. `legacy-demo/` preserves an archived static prototype. Architecture and infrastructure specifications include planned work; this README describes the current demo scope.
