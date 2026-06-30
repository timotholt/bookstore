# Davis's Books Tech Debt Register

Status: active working debt list.

This document tracks the highest-value debt we owe in the current Rust/Postgres app so we can prioritize by user impact, not by whatever is easiest to notice in the code.

## Priority Key

- `P0`: directly affects the current user experience or deployment viability.
- `P1`: strong user impact, but not the first thing to fix if we are only doing one slice.
- `P2`: worthwhile cleanup or follow-on architecture work.

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

### P1 - Homepage and book-detail read fan-out

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

### P1 - Cart mutation chatty-ness

Cart actions like add, remove, adjust quantity, save for later, and restore currently do more database work than the user action really requires.

Why it matters:

- These are the most repeated user interactions in the app.
- Each click does mutation work, then additional reads to rebuild the fragment.

Fix direction:

- Collapse each cart action into one internal application command.
- Make the command return the updated cart view data or the changed line.
- Reduce repeated cart-id and stock lookups inside one click path.

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

## Priority Order

If we are fixing one thing at a time, the order should be:

1. Put the app and Postgres close together for the real deployment path.
2. Collapse the cart command paths.
3. Parallelize homepage and book-detail reads.
4. Replace broad derived catalog reads with targeted queries.
5. Add better timing instrumentation.

