# Legacy Demo

This folder contains the original static Davis's Books prototype:

- `index.html`
- `script.js`

The current Rust/Axum storefront does not serve these files. They are kept only as a historical visual and interaction reference; their original branding is intentionally unchanged.

`script.js` is intentionally large because it powered the standalone demo: mock book data, client-rendered cards, localStorage cart state, search, filters, modal details, and carousel behavior. In the live app, those responsibilities now belong to Rust handlers, Askama templates, PostgreSQL seed data, HTMX fragments, sessions, and the smaller root `app.js`.
