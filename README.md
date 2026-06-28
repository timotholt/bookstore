# Davis's Books

Server-rendered Go + HTMX storefront for Davis's Books, backed locally by SQLite.

The live application entrypoint is `main.go`. The original static HTML/JS prototype is archived in `legacy-demo/` for visual reference only and is not served by the Go app.

## Run Locally

```bash
go mod tidy
go run .
```

The app listens on `http://localhost:8080` by default and creates `data/bookstore.db` automatically.

Optional environment variables:

```bash
ADDR=:8090
DATABASE_URL='file:data/bookstore.db?cache=shared&mode=rwc&_pragma=foreign_keys(1)'
APP_ENV=production
```

## Test

```bash
go test ./...
```

## Current MVP

- Go `chi` router and `html/template` rendering.
- Local SQLite schema and seed catalog with 28 used books.
- Server-rendered homepage shelves and catalog cards.
- HTMX catalog search/filter fragments.
- Session-backed cart drawer with quantity updates, stock caps, shipping math, and checkout placeholder.
- Local vendored HTMX runtime at `/assets/htmx.min.js`.
- Archived pre-migration demo under `legacy-demo/` for reference while backend migration continues.

## Next Production Integrations

- Replace checkout placeholder with Stripe Checkout session creation and webhook handling.
- Add staff auth and CMS inventory screens.
- Move production persistence to PostgreSQL and database-backed sessions.
