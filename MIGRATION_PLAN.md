# Technical Architecture & Implementation Plan: Go + PostgreSQL + HTMX

This specification outlines the transition of **Davis's Books** from a static mock frontend to a server-driven web application using a Go backend, PostgreSQL, and HTMX.

---

## 1. Core Technology Stack Specs

### Backend Language & Framework (Go)
* **Rationale**: Go compiles to a single, lightweight binary (~15MB), has sub-millisecond route dispatch, zero runtime dependencies, and a robust standard library.
* **Routing**: [go-chi/chi](https://github.com/go-chi/chi) (standard-library compatible, lightweight radix-tree router).
* **Templating**: Go standard library `html/template` or [a-h/templ](https://github.com/a-h/templ) (type-safe component templating). We will start with standard `html/template` for zero-compilation simplicity and clean integration with HTMX fragment queries.

### Database (PostgreSQL)
* **Rationale**: The industry-standard free relational database. It easily maintains sub-millisecond P95 response times under structured index maps.
* **Driver/ORM**: [jmoiron/sqlx](https://github.com/jmoiron/sqlx) (lightweight extension of Go's `database/sql` for struct scanning) or [jackc/pgx](https://github.com/jackc/pgx) (native PostgreSQL driver). We will avoid heavy, slow ORMs to maintain direct control over SQL query performance.
* **Local Development**: SQLite (zero-config, single-file database sharing identical SQL syntax structures).

### User Authentication & Sessions ("Just Works")
* **Rationale**: External SaaS auth engines add network latency and pricing overhead. A secure, self-hosted session manager keeps authentication entirely in-house.
* **Session Manager**: [alexedwards/scs](https://github.com/alexedwards/scs) (the gold-standard session manager for Go. Fast, database-backed, automatic cookie signing, secure rotation).
* **Password Hashing**: Go standard library `golang.org/x/crypto/bcrypt` or `argon2`.

### Payments & Shopping Cart Engine
* **Rationale**: Storing credit cards in-house introduces complex PCI-compliance audits.
* **Solution**: Keep the cart items in the database/session locally (in-house Cart model), and redirect the shopper to **Stripe Checkout** for payments. 
* **Flow**:
  1. User builds cart stack (saved locally in PostgreSQL sessions).
  2. User clicks checkout. Go backend requests a Stripe Checkout Session via Stripe API.
  3. User completes checkout on Stripe's secure page.
  4. Stripe sends a webhook request (`/webhooks/stripe`) to the Go server.
  5. Go server marks the transaction complete, sets the book copy to "Sold", and clears the cart session.

### The CMS & Inventory Management
* **Solution**: A custom admin panel built directly into the Go + HTMX binary. No third-party servers required.
* **Features**:
  - Staff login dashboard.
  - Scan or input ISBN.
  - Autofill book metadata (using standard Google Books REST API).
  - Add specific Used-Book grading: condition rating, formats checklist, pre-loved notes, and unique pricing.

---

## 2. Theme Switcher & CSS Variables System
Define theme tokens in [styles.css](file:///Users/timotholt/Documents/Davis's%20Books/styles.css) using CSS variables, toggled via a data attribute (`data-theme`) on the `<html>` node.

```css
:root, [data-theme="sage-rust"] {
  --ink: #1e2522;
  --paper: #eae6dd;
  --paper-strong: #dbd6cc;
  --surface: #ffffff;
  --line: #d2cbc0;
  
  --sage: #2a473c;
  --sage-dark: #162a22;
  --rust: #c2593f;
  
  --btn-add-bg: var(--sage);
  --btn-buy-bg: var(--rust);
}

[data-theme="cream-charcoal"] {
  --ink: #111111;
  --paper: #f4f1eb;
  --paper-strong: #e5dec9;
  --surface: #ffffff;
  --line: #d8cfb4;
  
  --sage: #333333;
  --sage-dark: #1a1a1a;
  --rust: #555555;
  
  --btn-add-bg: var(--sage);
  --btn-buy-bg: var(--rust);
}

[data-theme="coffee-parchment"] {
  --ink: #2c1a04;
  --paper: #f0e6d2;
  --paper-strong: #decbb7;
  --surface: #fffdf9;
  --line: #cdb9a6;
  
  --sage: #5c3a21;
  --sage-dark: #3a2211;
  --rust: #8b5a2b;
  
  --btn-add-bg: var(--sage);
  --btn-buy-bg: var(--rust);
}
```

---

## 3. Database Schema Blueprint (SQL)

```sql
-- Books Metadata (shared across copies)
CREATE TABLE books (
    id VARCHAR(50) PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    author VARCHAR(255) NOT NULL,
    genre VARCHAR(100) NOT NULL,
    year INTEGER NOT NULL,
    isbn VARCHAR(20) NOT NULL,
    cover_color VARCHAR(10) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Unique pre-owned physical copies
CREATE TABLE book_copies (
    id SERIAL PRIMARY KEY,
    book_id VARCHAR(50) REFERENCES books(id) ON DELETE CASCADE,
    condition VARCHAR(50) NOT NULL, -- 'Fine', 'Very Good', 'Good', 'Fair', 'Poor'
    price DECIMAL(10, 2) NOT NULL,
    notes TEXT,
    format VARCHAR(50) NOT NULL, -- 'Hardcover', 'Paperback', 'Trade Paperback'
    is_sold BOOLEAN DEFAULT FALSE,
    is_staff_pick BOOLEAN DEFAULT FALSE,
    staff_quote TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

---

## 4. Execution Roadmap

### Phase 1: Go Server Setup & Routing
* Create a Go project directory (`main.go`, `router/`, `handlers/`, `templates/`).
* Mount standard file server routes to serve assets and static CSS.
* Configure template engine parsing (`templates/*.html`).

### Phase 2: Database and Local Sessions
* Spin up SQLite local instance and create database schemas.
* Set up `alexedwards/scs` session configuration with SQLite memory-store.
* Seed tables with the 28 pre-owned catalog books.

### Phase 3: HTMX Page & Fragment Rendering
* Migrate HTML blocks into layouts (`layout.html`, `home.html`, `catalog.html`).
* Convert product cards into sub-templates (`components/card.html`).
* Write HTMX trigger points:
  - Catalog search query (`GET /catalog?q=dune`) returning only the catalog grid markup fragment.
  - Filter checklists triggering HTMX swaps on change event.

### Phase 4: Shopping Cart and Stripe Checkout
* Setup Stripe SDK configuration.
* Connect Cart addition/removal requests to Go endpoints using HTMX fragment updates.
* Configure Stripe checkout redirection route (`POST /checkout`).

### Phase 5: Auth, User Management, and CMS Panel
* Implement user login routes and session guards.
* Build the simple Admin CMS page for pre-loved inventory entries.
