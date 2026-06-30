# Davis's Books Product Architecture Spec

Status: canonical product and application architecture spec.

This spec supersedes the old migration plan. Davis's Books is a Rust/Axum, server-rendered commerce app for a used bookstore. The objective is to build a solid, interview-ready commerce system with real account, cart, review, and order foundations while staying simple enough to run locally without paid services.

## Objective

Build Davis's Books as a durable small-commerce storefront:

- Server-rendered Rust/Axum application.
- Askama templates with reusable include components.
- PostgreSQL persistence through `sqlx`, with Neon as the deployed demo database.
- HTMX for targeted swaps, not a separate client app.
- Account identity through email/password first, Google OAuth second.
- Persistent carts stored in the database.
- Product reviews with verified-purchase support.
- Review storage and aggregation follow `docs/REVIEWS_SPEC.md`.
- Local development can use Neon or any explicitly configured local PostgreSQL instance.
- No legacy Go server path.

The product should feel inspired by proven commerce patterns from large retailers, but scoped to a used bookstore. Copy the useful primitives, not the entire platform.

## Non-Goals

- Do not rebuild Amazon.
- Do not add a JavaScript SPA.
- Do not add Apple login until the account and paid developer requirements are worth it.
- Do not introduce a second database runtime to make local development work.
- Do not introduce a new web framework while Axum, Askama, HTMX, and `sqlx` are serving the job.
- Do not create parallel implementations when an existing module can be extended cleanly.

## Current Canonical Stack

- Rust 2021
- Axum for routing and request handling
- Askama for templates
- HTMX for server-rendered interactivity
- PostgreSQL with `sqlx` migrations
- `tower-sessions` for session management
- `tower-http` for static assets and middleware
- `argon2` for password hashing
- `uuid`, `chrono`, `serde`, `thiserror`, `tracing`

Prefer extending these libraries before adding new ones. New dependencies need a clear job, active maintenance, and a better risk profile than implementing the feature with the existing stack.

## Architecture Principles

1. Use the existing code first.
   Before adding a module, function, template, CSS class, or dependency, search for the existing equivalent and extend it when appropriate.

2. Keep contracts explicit.
   Data shape belongs in Rust models and SQL migrations. Presentation belongs in templates and CSS. Business rules belong in service/store layers, not duplicated across handlers and templates.

3. Keep handlers thin.
   Handlers should parse input, call focused domain functions, choose a response, and stop. Cart, auth, review, and order rules should move into dedicated modules as they grow.

4. Prefer boring state over clever repair.
   Persist carts, users, reviews, and identities explicitly. Do not rely on session-only state for things a user expects to survive a restart or login.

5. Templates include components.
   Askama macros are deprecated in this project because scoping was brittle in child templates. Use standalone include components that consume variables from scope.

6. CSS lives in CSS.
   Do not add inline styles to templates. Use classes in `styles.css`, backed by CSS custom properties in `:root` or a dedicated design-token section. If a value is reused or theme-related, make it a variable.

7. HTML stays semantic.
   Use forms, buttons, labels, fieldsets, anchors, and landmarks correctly. HTMX enhances server forms; it should not hide unclear markup.

8. Security is a feature, not polish.
   Auth, session, password, OAuth, and review moderation code must be deterministic, tested, and conservative.

9. UI patterns are mandatory.
   Do not build new UI as one-off markup. Every repeated UI pattern must have a Rust view object, a helper or constructor, one Askama include, one CSS class family, and optional HTMX/analytics attributes. New UI should use this pattern unless the existing component system cannot express the need.

## UI Pattern System

The UI should be boringly consistent. Cards, buttons, links, inputs, shelves, scroll areas, filters, and search controls should not be hand-built in every template. They should be rendered from strict Rust view objects through shared Askama include components and shared CSS classes.

### Required Component Contract

Every reusable UI component must have:

- A Rust view object that declares the data contract.
- A helper or constructor so callers do not fill boilerplate fields repeatedly.
- One Askama include template that renders the object.
- One CSS component class family in `styles.css`.
- Optional `AnalyticsAttrs` when the component is visible or interactive.
- Optional `HtmxAttrs` when the component performs an HTMX action.

Do not add a new button, card, input, link, search control, or scroll section by copying existing HTML and changing strings. First look for an existing view object and include. If none exists, add the reusable pattern and convert the immediate caller to it.

### Target Module Shape

As the UI pattern system is introduced, use this shape:

```text
src/ui/
  mod.rs
  actions.rs
  analytics.rs
  buttons.rs
  cards.rs
  forms.rs
  links.rs
  sections.rs

templates/components/ui/
  button.html
  link.html
  input.html
  select.html
  badge.html
  price.html
  empty_state.html
  product_card.html
  product_section.html
```

Do not create this whole tree before it is needed. Introduce files as patterns are converted, starting with product cards/sections and then forms/buttons/search.

### Canonical View Objects

The first reusable UI objects should be:

```text
AnalyticsAttrs
HtmxAttrs
LinkView
ButtonView
InputView
SelectView
BadgeView
PriceView
ProductCardView
ProductSectionView
EmptyStateView
ScrollAreaView
```

Example shape:

```rust
pub struct AnalyticsAttrs {
    pub event: &'static str,
    pub source: String,
    pub target_type: &'static str,
    pub target_id: String,
}

pub struct HtmxAttrs {
    pub method: HtmxMethod,
    pub url: String,
    pub target: String,
    pub swap: String,
}

pub struct ButtonView {
    pub label: String,
    pub variant: ButtonVariant,
    pub size: ButtonSize,
    pub disabled: bool,
    pub button_type: ButtonType,
    pub htmx: Option<HtmxAttrs>,
    pub analytics: Option<AnalyticsAttrs>,
}
```

Prefer helper constructors over direct struct literals at call sites:

```rust
ButtonView::primary("Add to cart")
    .htmx_post("/cart/items")
    .target("#cartDrawer")
    .track("add_to_cart_clicked", source, copy_id)
```

Domain helpers are even better when the action is common:

```rust
ui::buttons::add_to_cart(copy_id, source)
ui::links::book_detail(&book, source)
ui::cards::product_from_book(&book, source)
ui::sections::product_shelf("Best Sellers", books, "home.best_sellers")
```

### Askama Include Naming

Component includes should use stable variable names:

- `components/ui/button.html` expects `button`.
- `components/ui/link.html` expects `link`.
- `components/ui/input.html` expects `input`.
- `components/ui/product_card.html` expects `card`.
- `components/ui/product_section.html` expects `section`.
- `components/ui/empty_state.html` expects `empty_state`.

When looping, name the loop variable to match the include contract:

```html
{% for card in section.cards %}
  {% include "components/ui/product_card.html" %}
{% endfor %}
```

### Product Cards And Sections

Books, merch, catalog results, related books, staff picks, deals, and search results should converge on the same product card model:

```text
BookCard / Merch / SearchResult -> ProductCardView -> product_card.html
```

Sections should be data-driven:

```rust
ProductSectionView {
    id: "staff-picks",
    title: "Staff Picks",
    href: Some("/collections/staff-picks"),
    cards: product_cards_from_books(staff_picks, "home.staff_picks"),
    analytics_name: "home.staff_picks",
}
```

Adding a new product shelf should usually mean adding a new `ProductSectionView`, not writing new shelf markup.

### Tracking Attributes

Interactive and impression-worthy UI components should carry tracking metadata from the view object, not ad hoc template attributes:

```html
<article
  class="ui-product-card"
  data-track-impression="{{ card.analytics.event }}"
  data-source="{{ card.analytics.source }}"
  data-target-type="{{ card.analytics.target_type }}"
  data-target-id="{{ card.analytics.target_id }}"
>
```

Use first-party event tracking for product metrics such as searches, product views, cart actions, saved items, and reviews. Session replay tools may help diagnose behavior later, but the app's product events should not depend on replay tooling.

### CSS Class Families

UI CSS should use shared class families:

```css
.ui-button {}
.ui-button--primary {}
.ui-button--secondary {}
.ui-button--small {}
.ui-button--full {}
.ui-input {}
.ui-select {}
.ui-product-card {}
.ui-product-section {}
.ui-scroll-area {}
```

Use CSS custom properties for repeated values:

```css
:root {
  --ui-control-height: 40px;
  --ui-gap-sm: 8px;
  --ui-gap-md: 16px;
  --ui-radius-sm: 4px;
  --ui-radius-md: 8px;
}
```

Do not add one-off style attributes for spacing, layout, colors, typography, or responsive behavior.

## Domain Boundaries

### Catalog

Owns books, authors, genres, copies, variants, collections, and search filters.

Existing modules:

- `src/models.rs`
- `src/store.rs`
- `src/ui/mod.rs` product card and section view models
- catalog sections of `src/handlers.rs`
- `templates/components/book_cover.html`
- `templates/components/ui/product_card.html`
- `templates/components/ui/product_section.html`
- `templates/components/catalog_results.html`

Future direction:

- Split catalog queries into a `catalog` module only when tests exist.
- Keep one canonical query projection for reusable book cards.
- Add review aggregates to book detail and product cards through explicit query fields, not template-side lookups.

### Cart

Current state is session-backed. Target state is database-backed with anonymous and authenticated ownership.

Target behavior:

- Anonymous visitor gets a session cart.
- Logged-in user gets a user cart.
- On login, merge anonymous cart into the user's active cart.
- Quantities are capped by available stock.
- Removed, sold, or unavailable copies are dropped or marked unavailable.
- Cart survives server restart.

Target tables:

```text
carts
cart_items
```

Important fields:

```text
carts.id
carts.user_id nullable
carts.session_key nullable
carts.status active|converted|abandoned
carts.created_at
carts.updated_at

cart_items.cart_id
cart_items.copy_id
cart_items.quantity
cart_items.created_at
cart_items.updated_at
```

### Auth

Target auth model is one `users` table with many login identities.

Supported paths:

- Email/password
- Google OAuth/OpenID Connect

Deferred:

- Apple login, because it requires Apple developer setup and is not needed for the free local path.

Target tables:

```text
users
user_identities
password_credentials
```

Design:

- `users` is the account root.
- `user_identities` maps providers such as `google` or `password` to a user.
- `password_credentials` stores local password hashes separately from OAuth identity metadata.
- Email normalization belongs in one auth utility, not in handlers.
- Password hashes use `argon2`; never store raw passwords.
- Google login must validate `state`, expected issuer, audience, subject, email, and email verification.

Recommended libraries:

- Continue `tower-sessions`.
- Add a SQL-backed session store before account features go live.
- Use existing `argon2` dependency for password hashing unless a wrapper is adopted deliberately.
- Use the standard Rust `oauth2` crate for Google OAuth when implementing social login.

### Reviews

Reviews are tied to users and books. A review can be marked verified when linked to an order item for that book.

Target behavior:

- One review per user per book.
- 1-5 star rating.
- Optional title and body.
- Moderation status: pending, published, hidden.
- Verified purchase flag when backed by an order.
- Helpful votes.
- Report abuse.
- Sort by recent, highest, lowest, most helpful.

Target tables:

```text
reviews
review_votes
review_reports
```

Important fields:

```text
reviews.user_id
reviews.book_id
reviews.rating
reviews.title
reviews.body
reviews.status
reviews.is_verified_purchase
reviews.helpful_count
reviews.created_at
reviews.updated_at
```

Do not allow anonymous reviews. Do not allow duplicate reviews for the same user and book.

### Saved Items

Saved items are the small-bookstore version of wishlist and "save for later."

Target tables:

```text
saved_items
```

Behavior:

- Logged-in only.
- Save book or copy references depending on product decision.
- Used-copy scarcity means saving a specific copy may be useful, but saving the book is more stable.

### Orders

Orders are needed for verified-purchase reviews and eventual Stripe integration.

Target tables:

```text
orders
order_items
```

Behavior:

- Checkout converts cart to order.
- Order items snapshot title, copy id, price, condition, and format.
- Reviews use order items to determine verified purchase.

## Amazon-Inspired Feature Map

Build these in order:

1. Account identity.
2. Persistent cart.
3. Saved items.
4. Reviews and rating summary.
5. Helpful votes and review sorting.
6. Order history.
7. Verified-purchase badge.
8. Recently viewed books.
9. Basic recommendations from genre, author, collection, and cart context.

Avoid:

- Marketplace seller complexity.
- Ads.
- Dynamic pricing.
- Recommendation infrastructure beyond simple deterministic queries.
- Heavy client-side personalization.

## Implementation Order

### Phase 1: Foundation

- Introduce `AppState` with `db` and config.
- Add `/healthz` and `/readyz`.
- Add route-level integration tests.
- Move cart helpers out of `handlers.rs` into a cart module.

### Phase 2: Durable Sessions And Auth

- Add SQL-backed session storage.
- Add `users`, `user_identities`, and `password_credentials`.
- Add email/password signup, login, logout.
- Add account header state.
- Add auth tests for success, failure, logout, and duplicate account handling.

### Phase 3: Persistent Cart

- Add `carts` and `cart_items`.
- Preserve anonymous cart behavior.
- Merge anonymous cart into user cart on login.
- Update cart drawer and cart page from database cart state.
- Test stock caps, merge behavior, unavailable copies, and restart persistence.

### Phase 4: Google Login

- Add Google OAuth/OpenID Connect config.
- Add start and callback routes.
- Store provider identity in `user_identities`.
- Link Google identity to an existing account only with a safe verified-email policy.

### Phase 5: Reviews

- Add `reviews`, `review_votes`, and `review_reports`.
- Add review summary to book detail.
- Add create/edit/delete own review.
- Add helpful votes.
- Add moderation status.

### Phase 6: Orders And Verified Purchase

- Add `orders` and `order_items`.
- Convert checkout placeholder into order creation before Stripe.
- Mark matching reviews as verified purchase.
- Add order history page.

## Testing Requirements

Each phase needs tests before broader refactors:

- `cargo check`
- `cargo test`
- Route tests for core page responses.
- HTMX fragment tests for catalog/cart/review swaps.
- Auth tests for session fixation, logout, bad password, duplicate email, and OAuth state mismatch.
- Cart tests for anonymous cart, login merge, stock cap, and stale copy removal.
- Review tests for duplicate prevention, own-review editing, vote uniqueness, and moderation visibility.

## Styling Rules

- No new inline CSS in templates.
- Put reusable styles in `styles.css`.
- Put design values in CSS custom properties.
- New repeated UI must use the UI pattern system: Rust view object, helper, Askama include, CSS class family, and optional HTMX/analytics attributes.
- Prefer semantic class names tied to product concepts, such as `review-summary`, `account-menu`, `saved-item-list`.
- Keep HTMX state classes reusable.
- Do not create one-off style attributes for spacing, layout, colors, or typography.
- Use existing typography, radius, shadow, button, and surface variables before adding more.

## Documentation Rules

- `README.md` explains how to run, test, and find the canonical docs.
- This file owns product architecture and implementation order.
- `docs/INFRASTRUCTURE_SPEC.md` owns deployment, accounts, secrets, and provider recovery.
- Deprecated specs should say they are deprecated and link here.

## Definition Of Solid

A feature is solid when:

- It has one clear owner module.
- It extends existing models, templates, and CSS where possible.
- It has database constraints for invariants.
- It has tests for expected behavior and failure behavior.
- It survives server restart when users expect persistence.
- It does not require paid services for local development.
- It is documented in this spec or the infrastructure spec.
