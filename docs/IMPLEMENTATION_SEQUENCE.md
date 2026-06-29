# Implementation Sequence

Status: active execution order.

This document turns the product architecture spec into the current build sequence. Follow this order unless a real blocker or product decision changes the lane.

## Current Lane: UI Pattern Refactor First

Before adding auth, persistent carts, reviews, analytics providers, Neon, or deployment work, refactor the current UI into the shared pattern system from [PRODUCT_ARCHITECTURE_SPEC.md](PRODUCT_ARCHITECTURE_SPEC.md).

The goal is to avoid adding new product features on top of one-off markup and duplicated card/button/link behavior.

## Ordered Work

### 1. Commit/land architecture docs

- Keep `AGENTS.md` and `docs/PRODUCT_ARCHITECTURE_SPEC.md` as the working rules.
- Keep `MIGRATION_PLAN.md` deprecated.
- Keep `docs/INFRASTRUCTURE_SPEC.md` focused on deployment and secrets.

### 2. Product cards and sections

- Add `src/ui` view objects and helper constructors.
- Add shared `templates/components/ui/product_card.html`.
- Add shared `templates/components/ui/product_section.html`.
- Convert homepage product shelves first.
- Then convert catalog results and related books.
- Remove duplicated card templates once no longer used.

Initial progress:

- Homepage product shelves use `ProductSectionView` and `ProductCardView`.
- Catalog results use `ProductCardView`.
- Related books use `ProductCardView`.
- The old duplicated `product_tile.html` and `book_card.html` components have been retired.

### 3. Links and analytics attributes

- Add `LinkView`.
- Add `AnalyticsAttrs`.
- Render consistent `data-*` tracking attributes from the view objects.
- Do not persist analytics events yet; just standardize the markup contract.

Initial progress:

- Product card title links use `LinkView`.
- Product card add/buy buttons use `ButtonView`.
- Featured deal and book-detail buy box purchase buttons use `ButtonView`.
- Product cards render click and impression metadata from shared view objects.

### 4. Buttons and form controls

- Add `ButtonView`, `InputView`, and `SelectView`.
- Convert cart controls, catalog filters, search inputs, and account/auth forms as they are touched.
- Keep HTMX attributes in shared view objects instead of repeating them in templates.

Initial progress:

- Cart drawer line controls use `CartLineView` and shared `ButtonView`.
- Full cart page controls still need a dedicated page-fragment response before they can safely share drawer HTMX behavior.
- Full cart page static layout and typography styles moved from inline attributes into `styles.css`.
- Remaining cart inline styles are runtime values for cover color and progress width.

### 5. CSS cleanup

- Move inline styles from active templates into `styles.css`.
- Add `.ui-*` class families for shared controls and components.
- Use CSS custom properties for repeated design values.

Initial progress:

- Full cart page static styles moved into reusable cart page classes.

### 6. Server foundation

- Introduce `AppState`.
- Add `/healthz` and `/readyz`.
- Add route-level tests for home, catalog HTMX, book detail, and cart.
- Move cart helpers out of `handlers.rs`.

### 7. Persistent cart

- Add database-backed `carts` and `cart_items`.
- Preserve anonymous cart behavior.
- Merge anonymous carts into user carts on login.

### 8. Auth

- Add email/password auth first.
- Add Google OAuth/OpenID Connect second.
- Defer Apple login unless the project intentionally takes on Apple developer account requirements.

### 9. Reviews, saved items, and orders

- Add saved items.
- Add reviews and review votes.
- Add orders and order items.
- Use orders for verified-purchase reviews.

### 10. Tracking provider and deployment

- Add first-party analytics events before adding a replay/analytics provider.
- Decide between PostHog/OpenReplay/Plausible only after first-party event names exist.
- Integrate Neon/Railway after health checks, tests, and config shape are solid.

## Stop Rule

Do not start a later phase if the current phase would create more duplicate UI or repeated server behavior. Finish the shared pattern first, then add features on top of it.
