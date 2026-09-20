# Bookstore stack: study guide

Chantel's Corner builds HTML on a Rust server, sends it to the browser, and stores persistent data in PostgreSQL. This guide follows the project's current README and Cargo.toml.

## Learn these first

Pronunciations below are practical English approximations.

| Name | Say it | What it is | Role in the bookstore |
| --- | --- | --- | --- |
| Rust | rust | Programming language | Expresses application logic and data types. |
| Axum | ACK-sum | Web framework | Matches URLs and HTTP methods to request handlers. |
| Tokio | TOH-kee-oh | Async runtime | Runs asynchronous Rust work, including network and database operations. |
| Askama | ah-SKAH-mah | Template engine | Combines templates and Rust data to produce HTML. |
| HTML | H-T-M-L | Markup language | Describes headings, forms, links, and page structure. |
| CSS | C-S-S | Style language | Controls colors, spacing, typography, and layout. |
| HTMX | H-T-M-X | Browser JavaScript library | Sends requests from HTML controls and swaps returned HTML into the page. |
| SQL | S-Q-L, or sequel | Database query language | Describes what data to read or change. |
| SQLx | S-Q-L ex | Rust database toolkit | Sends SQL to PostgreSQL and maps returned rows into Rust values. |
| PostgreSQL | POST-gres cue ell | Database server | Stores books, accounts, carts, sessions, and other persistent records. Often called Postgres. |
| Cargo | CAR-go | Rust build and package tool | Downloads dependencies, builds the application, and runs tests. |

Memory line: **Axum routes. SQLx queries. PostgreSQL stores. Askama renders. HTMX swaps.**

## Follow one book search

1. A customer changes a search or filter in the browser.
2. HTMX sends an HTTP request to the Rust server for catalog results.
3. Axum routes the request to the matching handler.
4. The handler calls application/data-access code. SQLx sends a query to PostgreSQL.
5. PostgreSQL returns matching rows; SQLx maps them into Rust data.
6. The server prepares view data. Askama renders an HTML fragment containing the results.
7. The server returns that HTML. HTMX replaces the results area in the browser.

Tokio runs the asynchronous server work underneath this flow. It lets other tasks make progress while one operation waits for I/O; it does not automatically make every calculation faster.

In this project, `/catalog` supplies the HTMX fragment and `/search` supplies the full search page. A normal page visit can return a whole HTML document without an HTMX swap.

## Supporting tools

| Tool | Say it | Remember this |
| --- | --- | --- |
| Argon2 | AR-gon two | Hashes passwords so the database does not need the original password. Login verifies a password against its hash. |
| tower-sessions | tower sessions | Manages session state, with PostgreSQL storage in this project. A browser cookie connects requests to a session. |
| tower-http | tower H-T-T-P | Shared HTTP behavior, including serving static files and request tracing. |
| Serde | SER-dee | Converts between Rust data and serialized representations; serde_json handles JSON. |
| tracing | tracing | Records structured diagnostic events to help explain what the application did. |
| thiserror | this error | Helps define meaningful Rust error types. |
| UUID | U-U-I-D | Identifier format used for records. |
| Chrono | KROH-noh | Date and time library. |
| rust_decimal | rust decimal | Decimal arithmetic useful for monetary values. |

## Five distinctions worth remembering

- **Rust and Axum:** Rust is the language; Axum is a library written for that language.
- **SQL, SQLx, and PostgreSQL:** SQL is the query language, SQLx is the Rust interface, and PostgreSQL is the database that executes queries and stores records.
- **Askama and HTMX:** Askama runs on the server to generate HTML; HTMX runs in the browser to request and insert HTML.
- **Session and cart:** A session identifies a sequence of browser requests. Cart records store shopping selections. They are related but have different jobs.
- **Migration and query:** A migration is a versioned database change, such as adding a column. An ordinary query reads or changes data during app use.

SQLx offers compile-time checking through particular query macros. That does **not** mean every SQL string used with SQLx is automatically checked at compile time.

## Find it in the project

| Open this | Look for |
| --- | --- |
| `Cargo.toml` | Rust dependencies and requested versions. |
| `src/app.rs` | URL routes. |
| `src/handlers.rs` | Request coordination and responses. |
| `src/store.rs`, `src/cart.rs`, `src/auth.rs` | Data access and domain operations. |
| `src/ui/`, `src/templates.rs` | Rust data prepared for the UI and templates. |
| `templates/` | Askama HTML templates and reusable includes. |
| `styles.css` | Shared styling. |
| `migrations_postgres/` | Versioned database setup and changes. |

Current feature boundary: checkout previews the cart; it does not place orders or take payments. Carts are database-backed but currently keyed to the session/browser rather than durable account-owned carts. The README separates implemented features from plans.

## Test yourself

1. Which tool decides what code handles `/cart`?
2. Which tool stores a book after the Rust server restarts?
3. Which tool builds the book-card HTML?
4. Which tool replaces search results without reloading the whole page?
5. How are SQL, SQLx, and PostgreSQL different?
6. Which tool hashes passwords?
7. What runs asynchronous Rust tasks?
8. Where would you add a new database column?

**Answers:** 1. Axum. 2. PostgreSQL. 3. Askama, using prepared Rust data. 4. HTMX. 5. Query language, Rust database toolkit, database server. 6. Argon2. 7. Tokio. 8. A new SQLx migration.

Practice: explain the seven-step search flow aloud, then trace it through the files above.

## References

- Project: [README](../README.md), [dependencies](../Cargo.toml), [architecture](PRODUCT_ARCHITECTURE_SPEC.md).
- [SQLx documentation](https://docs.rs/sqlx/latest/sqlx/): queries, row mapping, migrations, and query macros. The project currently requests SQLx 0.8; latest documentation may describe a newer version.
- [HTMX documentation](https://htmx.org/docs/): requests and HTML swaps.
- [Tokio](https://tokio.rs/): asynchronous Rust runtime.
