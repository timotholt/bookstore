# Agent Guidance

This repository is a Rust/Axum, Askama, HTMX, SQLite storefront. The old Go server is retired. Do not reintroduce Go code or Go-era architecture.

## Source Of Truth

- Product architecture: `docs/PRODUCT_ARCHITECTURE_SPEC.md`
- Deployment and secrets: `docs/INFRASTRUCTURE_SPEC.md`
- Run/test basics: `README.md`

Read the relevant spec before making structural changes.

## Engineering Rules

- Search before adding. Use existing modules, helpers, templates, CSS classes, and dependencies before creating new ones.
- Do not duplicate business logic across handlers, templates, and JavaScript.
- Keep handlers thin. Move reusable cart, auth, review, order, and catalog rules into focused Rust modules once they grow beyond simple request orchestration.
- Use `sqlx` migrations for database shape. Do not hide persistent domain state in templates or session blobs.
- Preserve Askama include components. Do not bring back Askama macros for shared product/card rendering.
- New UI must use the project UI pattern system from `docs/PRODUCT_ARCHITECTURE_SPEC.md`: Rust view object, constructor/helper, Askama include, CSS class family, and optional HTMX/analytics attrs. Do not hand-build one-off buttons, links, cards, inputs, shelves, scroll areas, search controls, or merch/book sections when a reusable pattern exists or should exist.
- Prefer extending the current stack: Axum, Askama, HTMX, `sqlx`, `tower-sessions`, `tower-http`, `argon2`, `serde`, `thiserror`, `tracing`.
- Add a dependency only when it has one clear job, is actively maintained, and meaningfully reduces risk or complexity.
- Never store plaintext secrets, passwords, tokens, OAuth client secrets, or provider credentials in the repo.

## Styling Rules

- Do not add inline CSS to templates.
- Put styling in `styles.css`.
- Use CSS custom properties for colors, spacing, typography, shadows, radii, and theme-like values.
- Reuse existing design tokens before adding new ones.
- Use semantic class names that describe product roles, not one-off visual hacks.
- Prefer shared `.ui-*` class families for reusable controls and components.
- HTMX markup should enhance normal forms and links; keep HTML semantic and accessible.

## Feature Rules

- Account identity is the foundation for persistent cart, saved items, reviews, verified purchases, and order history.
- Implement email/password auth before Google login.
- Skip Apple login unless the project intentionally takes on Apple developer account requirements.
- Persistent carts belong in the database. Anonymous carts may be keyed by session; logged-in carts must attach to `users`.
- Reviews require logged-in users. One review per user per book. Verified-purchase status comes from orders.

## Verification

Run the narrowest meaningful checks after edits:

```bash
cargo check
cargo test
```

For web behavior, smoke-test the Rust server on a free local port:

```bash
ADDR=127.0.0.1:8081 cargo run
```

Then verify key routes such as `/`, `/catalog` with `HX-Request: true`, `/books/:id`, and `/cart`.
