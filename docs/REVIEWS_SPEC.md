# Davis's Books Reviews Spec

Status: design foundation. No public review UI is required yet.

## Goals

- Store one product review per logged-in user per book.
- Support visible review aggregates on product cards, product detail, catalog sorting, and staff/admin screens.
- Derive verified-purchase status from orders once checkout creates orders.
- Keep moderation explicit so unpublished reviews never leak into storefront aggregates.
- Keep review rules in Rust service/store code, not templates or browser JavaScript.

## Non-Goals

- Anonymous reviews.
- Reviews for individual used-copy condition rows. Reviews belong to the stable `books.id` product record.
- External review syndication.
- Client-side review aggregation.

## Data Model

`reviews` is the source of truth for submitted reviews.

- `book_id`: stable product id from `books`.
- `user_id`: required account id. It is text until the account table lands, then should become a foreign key or validated service contract.
- `rating`: integer 1-5.
- `title` and `body`: optional user copy, stored as text. Rendering must escape by default through Askama.
- `status`: `pending`, `published`, `rejected`, or `removed`.
- `verified_purchase`: cached boolean derived from orders.
- `verified_order_id`: optional order reference used to explain why the badge was granted.
- `helpful_count` and `unhelpful_count`: denormalized counters from `review_votes`.

`review_votes` stores one helpful/unhelpful vote per user per review.

`review_aggregates` stores the published-only summary for fast storefront reads:

- `published_count`
- `rating_sum`
- `average_rating`
- `star_1_count` through `star_5_count`
- `verified_count`

## Aggregation Rules

Only `status = 'published'` reviews count toward aggregates.

When a review is created, edited, moderated, removed, or has its rating changed, rebuild the aggregate for that `book_id` in one transaction:

1. Select all published reviews for the book.
2. Count total published reviews.
3. Sum ratings.
4. Count each star bucket.
5. Count published reviews with `verified_purchase = 1`.
6. Upsert `review_aggregates`.

If the published count is zero, keep an aggregate row with zero values or delete it. The recommended first implementation is to keep the zero row because callers can join without special cases.

## Verified Purchase

Verified-purchase status is not user editable.

Once orders exist, a review is verified when:

- The review `user_id` matches the order `user_id`.
- The order has a completed paid or fulfilled status.
- At least one order line references a `book_copies.id` whose `book_id` matches the review `book_id`.

The service should set `verified_purchase` and `verified_order_id` when creating the review, and should also offer a backfill job for older reviews after order data is introduced.

## Store API Shape

Add a focused review module once handlers are needed:

- `reviews::submit_review(db, user_id, input)`: validates one-review-per-user, rating range, body length, and verified purchase.
- `reviews::moderate_review(db, review_id, status)`: changes status and rebuilds the aggregate.
- `reviews::review_summary(db, book_id)`: returns aggregate with zero defaults.
- `reviews::reviews_for_book(db, book_id, page)`: returns published reviews only.
- `reviews::vote_review(db, review_id, user_id, vote)`: upserts a vote and updates helpful counters.

Handlers should only parse forms, call the review service, and return full pages or HTMX fragments.

## UI Plan

No UI is required for this phase. When added, follow the project UI pattern system:

- Rust view object: `ReviewSummaryView`, `ReviewCardView`, `ReviewFormView`.
- Askama includes: `components/ui/review_summary.html`, `components/ui/review_card.html`, `components/ui/review_form.html`.
- CSS family: `.ui-review-*`.
- Product cards should consume `ReviewSummaryView` rather than hard-coded rating counts.

## Moderation Policy

The first implementation can default new reviews to `pending`. Admin tooling can later publish or reject them.

If the product needs lower-friction local demos, development seed data may insert published reviews, but user-submitted reviews should still go through explicit status transitions.

## Migration Path

The initial migration creates `reviews`, `review_votes`, and `review_aggregates` without foreign keys to users or orders because those tables do not exist yet.

When account and order tables land:

1. Tighten `reviews.user_id` and `review_votes.user_id` to the canonical user id contract.
2. Tighten or validate `verified_order_id` against orders.
3. Add order-line lookup in the review service.
4. Backfill verified-purchase flags.
5. Replace placeholder product-card rating counts with real `review_aggregates` reads.
