# Architecture review — Astra — 2026-10-14

Scope: existing Rust storefront/review architecture and proposed shared account/staff administration. Performed by a read-only GPT-6 Astra agent at the user's request, coordinated with “Plan admin dashboards.” Code/spec inspection and PostgreSQL locking documentation were used; no tests, hosted database operations, or catalog changes were performed by the reviewer. Concurrency findings below are source-derived interleavings, not reproduced incidents.

## Decision

Keep Rust/Axum, PostgreSQL/SQLx, Askama, HTMX, and database sessions. The architecture is suitable for user administration and eventual order dashboards. The missing seam is a common account lifecycle/authorization contract, not a new framework or database. Keep focused reviews, account administration, authorization, and eventual order modules inside the existing application.

The original admin plan was directionally sound but not ready to implement unchanged. `ADMIN_ACCESS_SPEC.md` has now been corrected around the findings. The historical findings below have now been addressed in source and local regression tests. This report records the original inspection; line references describe that snapshot.

## Existing implementation findings

| Priority | Finding | Evidence | Correction |
|---|---|---|---|
| P1 | Request-authenticated review/profile writes can commit after password-reset revocation commits. Review services accept user IDs without a session epoch; moderator existence reads do not serialize revocation. | `src/review_web.rs:453`, `src/reviews.rs:183`, `:272`, `:301`, `:401`; `src/auth.rs:307`. `src/app.rs:2744` tests a request started after revocation, not an in-flight request. | Pass an authenticated principal/security epoch into domain mutations and validate under an account lock in the same transaction. Preserve distinct token authority for unauthenticated recovery. Test both lock orderings deterministically. |
| P2 | Marketing-preferences submission writes back an earlier snapshot of all profile fields, potentially undoing a concurrent staff/customer profile correction. | `src/handlers.rs:1362`; `src/auth.rs:336`. | A narrow preferences update; expected revision for competing profile changes. |
| P2 | Physical user cascades acquire review rows before an AFTER trigger acquires review-work locks, opposite ordinary service order, permitting deadlock. | `src/reviews.rs:253`; `migrations_postgres/20261014000000_review_workflows.sql:85`, `:99`; sequential cascade tests at `src/app.rs:2883`. | Keep dashboard account deletion out of scope. Before supporting deletion, use explicit ordered cleanup and concurrent deletion/mutation proof. Do not assume trigger aggregation supplies the locking protocol. |
| P2 | Review identity trims ordinary spaces before collapsing other whitespace; edge tabs/newlines remain in the normalized hash, and whitespace-only authors can miss the missing-author fallback. | `migrations_postgres/20261014000000_review_workflows.sql:10`. | Collapse whitespace, then trim and test author emptiness. Add tabs/newlines fixtures. Adjust review identity through a migration, without changing catalog records. |
| P2 | Cross-replica rating-filtered catalog counts can remain stale for five minutes while review/result rows use thirty seconds. | `src/store.rs:439`; `src/catalog_cache.rs:47`; `docs/REVIEWS_SPEC.md` consistency section. | Use the shorter lifetime for rating-dependent counts and test two cache instances against the declared convergence bound. |

## Shared-admin design corrections

1. Staff targets require both `users.manage` and `staff.manage` for administrative status, profile, session, and security changes. A support-only account must not re-enable disabled administrators whose retained roles would restore privileged access. Target roles are checked inside the locked transaction. Normal self-service profile editing is separate.
2. Ordinary routes prohibit all self-role assignment changes, including administrator self-grants. Other administrators or trusted audited operator bootstrap/recovery perform those changes. Role combinations remain explicit; administrator status does not silently bypass domain permissions.
3. Reuse the single `auth_version` security epoch, explicitly accepting that role/status/session-revocation actions also invalidate pending verification/reset/email-change challenges. Disabled accounts cannot issue/consume account challenges; recovery retains unauthenticated token semantics and generic responses. Re-enable never silently restores old sessions or challenges.
4. Cover login, profile/preferences, email-token issuance/consumption, all review writes, and operator commands with the shared contract. A request check alone is insufficient for consequential mutations.
5. Preserve at least one active account combining `staff.manage` and `users.manage`; counting the Administrator role alone can leave no account able to restore disabled staff. Protect removal of either capability under the same mutex. Acquire one administration mutex before account rows for bootstrap/recovery, role changes, and all administrative status changes. Sort distinct account IDs; take the strongest needed row mode immediately (`FOR SHARE` for read-only authority checks; `FOR UPDATE` for account writes). Recheck actor/target/invariants, then take domain locks. Do not upgrade shared account locks later. Preserve account → token and account → review work → review order.
6. Bootstrap/recovery targets existing active accounts, is idempotent, identifies the operator/reason, and audits atomically. Normal routes have no last-admin bypass. This supersedes the infrastructure document's older credential-bootstrap proposal.
7. Coordinate schema/backfill, application/CLI cutover, and exclusion of obsolete moderator writers. Do not assume a one-time role copy plus rolling old processes creates a single authority. The review implementation has not yet been deployed, so a coordinated cutover is the simplest current plan.
8. General audit uses typed, minimal metadata and survives account deletion. Define audit-reading permissions. Initially retain minimal events with no automatic purge; any later purge policy must specify interval and semantics. Do not copy review-specific cascading audit deletion into general administrative history.

These corrections are recorded in the coordinated admin spec. “Plan admin dashboards” owns the common authorization/account/admin/audit implementation. “Plan reviews and checkout pause” owns adoption by review guards and its focused tests once the shared helper exists. The other chat reviewed these corrections and confirmed no blocking design conflict after adding the combined-capability administrator invariant. Implementation was subsequently authorized and completed collaboratively.

## Boundaries to retain

- One stable review identity per account/title/primary-author work, immutable submitted revisions, published snapshots, and exact revision checks.
- Published-only rating aggregates, one authoritative PostgreSQL database, and normal server-rendered forms.
- Explicit eventual consistency for process-local public caches; sensitive-content removal with immediate cross-replica guarantees would need an additional product/invalidation contract.
- An explicit work-identity/reassignment policy before any future catalog title/author editing: computed keys would otherwise detach existing reviews. Current catalog records remain protected.
- Future orders require authoritative order/item snapshots, separate payment/fulfillment state, stock allocation, idempotent provider events, and refund rules. Permissions and dashboard controls do not implement those domain workflows.

## Required acceptance proof

- Role/target matrix, support-only attempts against disabled staff, and all self-role cases.
- Deterministic blocked-transaction revocation tests across reviews, profile/preferences, and account-email flows.
- Concurrent administrator disable/demotion, role grant versus disable, and actor/target reversal without lock-upgrade deadlocks.
- Bootstrap/recovery idempotency, mutation/audit rollback, audit access and retention behavior.
- Migrated moderator preservation and exclusion of obsolete CLI/table writers.
- Existing review lifecycle, exact ratings, edge-whitespace grouping, unchanged catalog fingerprints.
- Two cache instances demonstrating the documented convergence limits.

Locking reference: [PostgreSQL explicit locking](https://www.postgresql.org/docs/current/explicit-locking.html), including consistent lock ordering and acquiring the most restrictive lock first.

## Resolution and follow-up review

Implemented shared principal/security-epoch checks under transaction-held account locks; narrow preference updates and optimistic profile versions in both staff and self-service forms; rejection of physical user deletion; review-only identity normalization/backfill preserving revisions; and thirty-second rating-dependent count expiry. Shared administration now enforces staff-target boundaries, additive roles, stable account locking, the combined-capability last-administrator invariant, audited operator recovery, and independent account/staff audit event allowlists.

Focused deterministic PostgreSQL tests cover both revocation/write orderings, admin concurrency, profile conflicts, email authority, migration preservation, and two cache instances. Local browser suites cover account administration, review lifecycle, checkout pause, role isolation, audit visibility, and catalog fingerprints.

Astra performed a fresh targeted source review after implementation and confirmed the audit allowlists and self-service profile conflict fixes, reporting no remaining blocking defect in that targeted review. The reviewer did not run tests; runtime evidence comes from the implementing chats. This is local verification, not a hosted deployment claim.
