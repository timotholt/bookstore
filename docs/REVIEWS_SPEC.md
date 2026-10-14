# Chantel's Corner Reviews

Status: implemented. Reviews require an authenticated account. Public reading does not.

## Identity and catalog protection

Reviews describe a title and its primary author across ISBNs and sellable copies. `review_title_key` hashes the normalized title and primary-author name (collapse whitespace, trim, lowercase). Punctuation, subtitles, and author spelling differences are intentionally preserved: no fuzzy merging. Missing authors use a book-specific fallback to avoid merging unrelated anonymous works. This follows the catalog's existing primary-author contract; books with the same title and primary author share reviews.

`review_works` stores review-only identities. The migration and runtime never update books, authors, ISBNs, copies, stock, prices, or other catalog records. The new migration clears legacy reviews/votes/aggregates, as authorized, without touching catalog data. Zero published reviews displays “No reviews yet,” not invented stars. Test fixtures run only in an isolated local PostgreSQL schema.

## Storage

- `reviews`: stable account/work identity; one per account/work; source book link; current and published revision pointers; public snapshot; deletion timestamp.
- `review_revisions`: rating 1–5, optional title (120 Unicode characters), optional body (5,000), submission time, moderation status and reason. Composite foreign keys prevent pointing at another review's revision.
- `user_staff_roles`: shared account authority; Moderator grants `reviews.moderate`. The migration preserves old moderator grants and retires `review_moderators`. Signup grants no privileges.
- `review_moderation_events`: review/revision, staff actor, decision, reason, timestamp.
- `review_reports`: signed-in reporter, exact published revision, reason, open/resolved state. One report per account/revision.
- `review_votes`: one helpful vote per account/review. Authors cannot vote for themselves; votes can be withdrawn. Publishing replacement text clears endorsements of the old version and resolves reports about the superseded public version with an audit event.
- `review_work_aggregates`: rebuildable published count, sum, arithmetic average, and star distribution for the work. The legacy per-book aggregate table is no longer read.

Published snapshots are updated only by moderation. Disabling an account preserves published reviews; physical account deletion is rejected. Author review deletion immediately withdraws the review and scrubs all stored revision title/body/reason text and its public snapshot; minimal ratings/IDs and moderation decisions remain for audit. Deleted reviews cannot be restored by an old staff form. A subsequent author submission reuses the identity and enters moderation.

## Workflow

New submission → pending → published or rejected.

An author edit creates a pending revision while the approved version remains public and continues to supply its rating. Approval atomically replaces that version. Rejection preserves the previous approved version. Removal withdraws the published version and rejects the current revision with a reason; the author may submit a replacement for moderation.

Staff inspect a detail page containing the latest revision, currently published version, latest 20 reports, and latest 20 audit events. Approve/reject require a pending revision; reject/remove require a reason. Staff cannot approve or reject their own submissions. All actions carry the exact current revision ID, preventing decisions based on stale text. Reports alone do not hide reviews.

The public policy is sentiment-neutral: remove spam, harassment, personal information, and irrelevant content; do not remove honest criticism because it has low stars. Moderators cannot rewrite customer text.

## Account and public UI

- Book detail: actual stars/count, five-star distribution, paginated published reviews, newest/highest/lowest/helpful sorting, sign-in-gated author form, helpful voting and reporting.
- `/account/reviews`: all active reviews belonging to the signed-in account, status/rejection reason, edit link, and explicit delete confirmation.
- `/books/:book_id/reviews/edit`: add/edit form with retained fields on validation failure. Login required.
- `/staff/reviews`: pending/published/reported/rejected/removed queues; staff only.
- `/staff/reviews/:id`: inspect and moderate; staff only.

Forms use normal HTML POST/redirect/GET. They do not depend on JavaScript or HTMX to persist edits. Product cards, homepage featured deal, and book detail use one `ReviewSummaryView` include. Form fields and buttons use shared view objects/includes. All text is escaped by Askama. Private review pages send `Cache-Control: private, no-store`.

## Consistency and abuse controls

The server obtains author IDs from the authenticated session; submitted author IDs have no authority. Own-review reads and writes are account-scoped. CSRF tokens and matching Origin are required on every review mutation. Moderator permission is checked again inside moderation transactions.

All review mutations validate an active principal and security epoch under an account lock before locking the shared work row and review row. A database trigger rebuilds published-only aggregates in the same transaction. Expected revision checks protect against stale edits/deletions/moderation. PostgreSQL constraints enforce ownership references, star ranges, and uniqueness. Account submission quotas allow five revisions/minute; reports allow ten/hour, using transaction-held account locks to serialize quota checks.

Public work identities, summaries, and lists use the existing bounded catalog cache. Successful POSTs invalidate the current process's cache. Other replicas converge under existing TTLs (public review summaries/lists and rating-dependent catalog counts: 30 seconds). Private author/staff lists are never cached. Database reads use explicit row budgets and pagination.

Verified-purchase badges are intentionally absent: this application currently has no authoritative paid/fulfilled order records. Email verification is not purchase verification.

## Staff setup

Use an existing account. On the intended database, after reviewing pending migrations:

```bash
cargo run --locked -- --grant-review-moderator staff@example.com operator-name "Grant moderation"
cargo run --locked -- --revoke-review-moderator staff@example.com operator-name "Revoke moderation"
```

These administrative commands require database/operator access and run pending migrations before updating permission. They never make an ordinary public signup a moderator. Role changes invalidate existing sessions/challenges; transactional guards serialize revocation with in-flight actions. Do not use a production database for tests.

## Ordering pause

GET `/checkout` displays the ownership-transition notice, including with an empty cart. The cart remains usable. POST `/checkout` returns HTTP 409 with the same notice; no order, payment, or reservation is created. Payment/pickup preview controls are removed. The cart and drawer flag that new orders are paused. The checkout notice includes a clickable customer-service phone number, (383) 383-4446, for questions and refund requests.

The existing-order fulfillment promise refers to orders handled by the business outside this application's current order-history implementation. Reopening ordering requires an explicit implementation change and an actual order/payment workflow, not simply hiding the warning.

## Validation

`cargo check --locked` and local PostgreSQL `cargo test --locked -- --test-threads=1`. New lifecycle tests exercise real sessions, CSRF, author ownership, staff permissions, public visibility, voting/reporting, stale revisions, edits, rejection, removal, deletion, ordering pause, and catalog fingerprints. Separate tests cover title/author edition grouping, concurrent edits/approvals, pagination, database constraints, and rejection of physical account deletion.

Design references: [Trustpilot author ownership](https://legal.trustpilot.com/forreviewers/guidelines-for-reviewers), [Bazaarvoice authenticity policy](https://www.bazaarvoice.com/legal/authenticity-policy/), [OWASP object authorization](https://cheatsheetseries.owasp.org/cheatsheets/Insecure_Direct_Object_Reference_Prevention_Cheat_Sheet.html), [PostgreSQL locking](https://www.postgresql.org/docs/current/explicit-locking.html). Revision behavior is this application's explicit product policy.

Browser verification: `scripts/test_reviews_browser.cjs` requires Playwright, an isolated local `DATABASE_URL`, and a running local server with `PUBLIC_BASE_URL` matching `REVIEW_TEST_BASE_URL` (default `http://127.0.0.1:8087`). Signup requires the existing email service in local capture mode; do not use live delivery. `REVIEW_TEST_CHROME` can select an installed Chrome executable. Screenshots default to `/tmp/chantels-review-browser`. The script creates test accounts and temporary review content, grants one local test moderator through the administrative command, and deletes its review after exercising the lifecycle. Author actions are verified with JavaScript disabled.

## Shared admin integration

Moderators use normal account login and the shared `access` permission helpers. User administration at `/staff/users` manages profiles, account status, session revocation, and additive staff roles with an audit trail. Only authorized administrators assign roles; moderator access alone grants no profile, order, refund, or role-management permissions. See `ADMIN_ACCESS_SPEC.md` for setup and permission rules.

The normalization migration changes review-owned identities only, preserving reviews and revision IDs. If normalization would collide with two reviews by the same author, migration aborts rather than discarding either review. Catalog title/author edits remain outside this scope.
