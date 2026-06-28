# Legacy Demo

This folder contains the original static Davis's Books prototype:

- `index.html`
- `script.js`

The Go + HTMX storefront does not serve these files. They are kept only as a visual and interaction reference while migration continues.

`script.js` is intentionally large because it powered the standalone demo: mock book data, client-rendered cards, localStorage cart state, search, filters, modal details, and carousel behavior. In the live app, those responsibilities now belong to Go handlers, templates, SQLite seed data, HTMX fragments, sessions, and the smaller root `app.js`.
