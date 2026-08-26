# Davis's Books Tech Debt Register

Status: active working debt list.

This document tracks the highest-value debt we owe in the current Rust/Postgres app so we can prioritize by user impact and by the canonical specs, not by whatever is easiest to notice in the code.

## Priority Key

- `P0`: directly affects the current user experience or deployment viability.
- `P1`: strong user impact, but not the first thing to fix if we are only doing one slice.
- `P2`: worthwhile cleanup, missing product foundation, or follow-on architecture work.

## Current Debt

### P0 - Remote Postgres latency on the hot path

The local app is speaking to Neon over the network for every page and cart interaction. That means the browser is waiting on remote database round trips before HTML can render.

Observed impact:

- `GET /readyz` is around 70-110 ms.
- `GET /` is around 375-455 ms.
- `GET /books/b003` is around 368-550 ms.
- cart `+` / `-` clicks feel even heavier because they do write + read work.

Why it matters:

- This affects every interaction.
- It makes the app feel slow even when the Rust server itself is healthy.

Fix direction:

- Run the app near Neon in deployment.
- Use a local Postgres instance for fast local development when we want speed without changing the database contract.

### P1 - Missing account foundation

The product architecture spec treats account identity as the foundation for persistent carts, saved items, reviews, verified purchases, and order history. We have not built that foundation yet.

Missing pieces called for by the specs:

- `users`
- `user_identities`
- `password_credentials`
- email/password signup, login, and logout
- SQL-backed session storage before account features go live
- account header state
- Google OAuth/OpenID Connect later, after password auth exists

Why it matters:

- Several later features depend on a real user identity model.
- Without this layer, carts, reviews, and order history stay fragmented.

Fix direction:

- Land email/password auth first.
- Keep the user root in `users`, with provider identities split out into `user_identities`.
- Move session persistence onto Postgres before broadening the auth surface.

### P1 - Checkout and orders are still placeholder-only

The app has a checkout page, but the real purchase flow still needs orders, order items, and payment handoff.

Missing pieces called for by the specs:

- `orders`
- `order_items`
- server-side checkout session creation
- order creation before redirect
- payment webhook handling
- checkout success and cancel pages
- order history

Why it matters:

- Orders are required for verified-purchase reviews.
- Checkout is the line between browsing and actually buying.

Fix direction:

- Convert the checkout placeholder into a real order flow.
- Keep payment handling on Stripe, with the app owning cart, order, and inventory state.

### P1 - Cart mutation chatty-ness

Cart actions like add, remove, adjust quantity, save for later, and restore currently do more database work than the user action really requires.

Why it matters:

- These are the most repeated user interactions in the app.
- Each click does mutation work, then additional reads to rebuild the fragment.

Fix direction:

- Collapse each cart action into one internal application command.
- Make the command return the updated cart view data or the changed line.
- Reduce repeated cart-id and stock lookups inside one click path.

### P2 - Reviews, helpful votes, and moderation are still unbuilt

The review spec exists, but the actual review system is not in place yet.

Missing pieces called for by the specs:

- `reviews`
- `review_votes`
- `review_reports`
- review aggregates / reviewer scores
- verified-purchase derivation from orders
- one-review-per-user-per-book enforcement
- review sorting by recent, highest, lowest, and most helpful
- review UI on product detail and product cards

Why it matters:

- Reviews are part of the intended commerce foundation, not a nice extra.
- The spec expects aggregate ratings to show up in product views.

Fix direction:

- Build the Rust service/store layer first.
- Keep moderation explicit and make aggregates come from the database, not templates.

### P2 - Staff auth and CMS inventory workflows are missing

The infrastructure spec calls for a real staff/admin surface, and we have not built it yet.

Missing pieces called for by the specs:

- staff login page
- admin guard middleware
- inventory list
- create/edit book metadata
- create/edit physical copy records
- mark copies sold or unavailable
- staff pick flag and quote editor
- collection manager for homepage shelves
- cache invalidation after writes

Why it matters:

- The store needs a way to maintain catalog data without manual database edits.
- This is a core part of the interview-ready demo story.

Fix direction:

- Build the minimum staff auth surface first.
- Add catalog CRUD behind the guard, then wire in cache invalidation.

### P2 - Saved items are still missing

The product architecture spec includes saved items as a distinct concept from cart.

Missing pieces called for by the specs:

- `saved_items`
- logged-in-only save and restore flows
- save book or copy behavior, depending on product decision

Why it matters:

- Saved items are the used-book equivalent of wishlist and "save for later."
- They become more useful once accounts exist.

Fix direction:

- Add them after account identity and persistent cart are stable.

### P2 - Later discovery features from the spec are still missing

The product architecture feature map includes a few follow-ons that are lower priority than the core commerce path, but they are still part of the intended product shape.

Missing pieces called for by the specs:

- recently viewed books
- basic recommendations from genre, author, collection, and cart context

Why it matters:

- These are the next layer of product discovery once the core account, cart, and order flows are solid.

Fix direction:

- Treat these as follow-ons after the core commerce paths and staff tooling are stable.

### P2 - Public-read query shape still fans out too much

`/` and `/books/:id` currently do too many sequential reads before they can render.

Examples:

- homepage reads filters, default catalog, best sellers, deals, staff picks, cart, and removed-notice state.
- book detail reads the book, copies, variant attributes, full catalog for related books, cart, and removed-notice state.

Why it matters:

- One request can fan out into several DB calls before the first byte is sent.
- Remote DB latency multiplies because the reads are mostly serialized.

Fix direction:

- Parallelize independent reads with `tokio::try_join!`.
- Replace broad catalog reads with targeted queries for related items.
- Skip cart-related reads when the request does not need them.

### P2 - Full-catalog reads for derived UI state

Some pages fetch the full catalog just to compute things like filters, related books, or shelf counts.

Why it matters:

- It is not the biggest latency driver today, but it is extra work that compounds with remote DB access.

Fix direction:

- Replace derived uses of `list_books(default)` with targeted summary queries.
- Cache stable shelf/filter metadata where it does not need to be recomputed on every request.

### P2 - Measurement gaps around query timing

We have route timing, but not enough per-query timing in the normal app path.

Why it matters:

- It makes it harder to tell whether the problem is query shape, DB distance, or both.

Fix direction:

- Add lightweight tracing around the biggest page loaders and cart commands.
- Track total route time and per-loader time separately.

### P2 - First-party analytics provider integration is still deferred

The app has first-party event vocabulary and storage work, but the actual external tracking/provider layer is still not part of the build.

Why it matters:

- We want product events to be intentional before wiring in a third-party analytics surface.

Fix direction:

- Keep first-party event names stable.
- Add a provider only after the product events are proven useful.

### P2 - Behavioral analytics is not instrumented

We capture explicit clicks and searches, but we do not measure mouse movement, dwell time, scroll depth, or other richer engagement signals.

Why it matters:

- It is easy to mistake click analytics for full behavioral analytics.
- If we later want engagement analysis, we will need a deliberate instrumentation plan instead of ad hoc scripts.

Fix direction:

- Treat behavioral analytics as a separate decision from first-party click tracking.
- Add only the signals we can explain, validate, and use.

## Priority Order

If we are fixing one thing at a time, the order should be:

1. Put the app and Postgres close together for the real deployment path.
2. Land the account foundation: users, identities, password auth, and SQL-backed sessions.
3. Convert checkout into real orders and order items.
4. Collapse the cart command paths.
5. Add reviews, review votes, moderation, and verified-purchase support.
6. Add staff auth and CMS inventory workflows.
7. Parallelize homepage and book-detail reads.
8. Replace broad derived catalog reads with targeted queries.
9. Add better timing instrumentation.
10. Add saved items and the lower-priority Amazon-inspired follow-ons.
11. Decide whether behavioral analytics is actually worth the instrumentation cost.
