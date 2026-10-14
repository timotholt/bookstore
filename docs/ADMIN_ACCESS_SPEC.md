# Shared account and staff access

Status: implemented and verified locally after Astra architecture review. See `docs/ARCHITECTURE_REVIEW.md` for findings and resolution evidence. This contract joins the work in “Plan admin dashboards” and “Plan reviews and checkout pause.” It replaces review-specific authorization, without changing existing catalog records.

## Login and authority

Customers and staff use the existing email/password login and account identity. Signup grants no staff role. Account management grants or revokes roles for existing accounts; there is no public staff signup or email-domain privilege rule.

`user_staff_roles(user_id, role)` is the single database-backed authority, with a foreign key to users, unique account/role pairs, and constrained role names. Explicit permission mappings live in Rust; roles can be combined. Do not maintain independently editable moderator grants after migration.

| Role | Permissions | Excluded unless separately granted |
|---|---|---|
| Moderator | `reviews.moderate`: review queues, approval/rejection/removal, reports | Profiles, orders, shipments, refunds, staff assignments |
| User support | `users.view`, `users.manage`: permitted profile changes, account status, session revocation | Staff assignments, review moderation, refunds, fulfillment |
| Fulfillment | `orders.view`, `orders.fulfill`: shipment allocation, holds, shipment updates | Refund execution, staff assignments, profile administration |
| Refund operator | `orders.view`, `refunds.issue`: refunds within explicit order/payment rules | Fulfillment, staff assignments, review moderation |
| Administrator | `staff.manage`: grant/revoke staff roles and access | Other domain permissions unless explicitly assigned |

An administrator managing staff status also needs the relevant user-management permission. The bootstrap operator can assign both Administrator and User support to the first administrator. Only holders of `staff.manage` may assign staff roles. Supporting a customer does not authorize granting that customer staff privileges.

Administrative profile, status, and session changes targeting any account with staff roles require **both `users.manage` and `staff.manage`**, including enable/disable, forced session revocation, profile corrections, and security actions. Role assignment itself requires `staff.manage`; it does not additionally require `users.manage`. Determine target classification under transaction locks, not from an earlier unlocked lookup. A disabled staff account retains its roles, so re-enabling it restores privileges and requires the same protection. Normal self-service profile editing is separate from staff administration.

Ordinary administration routes prohibit changing one's own staff-role assignments, including self-grants by administrators. Another administrator may assign roles. Trusted operator bootstrap/recovery commands are a separate, audited path that cannot be invoked through public routes. This deliberately limits self-escalation even for `staff.manage` holders.

## Account status and sessions

Accounts have `active`/`disabled` status and status-change metadata (actor, reason, time). Preserve orders, reviews, and other history when disabling an account. Public reviews remain visible unless a separate moderation action removes them.

Disabling or changing staff roles increments `users.auth_version` atomically. All prior sessions and outstanding verification, password-reset, and email-change challenges are invalidated because those challenges also bind `auth_version`. This single security epoch is intentional. Re-enabling requires fresh login and new challenges where needed. No second version counter is introduced. Active status and current permissions are checked on each protected request and again inside consequential write transactions. Never trust role flags from a browser or stale session. Guest public browsing remains available.

Use shared Rust authorization helpers: request-level `require_permission`, transaction-level `require_permission_tx`, and an active-account transaction check for ordinary authenticated actions without a staff permission. Pass the authenticated account and session version explicitly. Login and every account mutation must adopt the contract: profile and preferences changes, verification resends, change-email issuance/confirmation, password-reset issuance/confirmation, all review submissions/edits/deletions/votes/reports/moderation, and operator role/status commands. Authenticated mutations recheck active status/session version inside their write transaction. Unauthenticated recovery keeps its existing token-based authority: lock the account, require active status, and validate purpose, target, expiry, consumption/revocation, and the current security epoch before consuming the token. Disabled accounts cannot issue or consume account challenges; recovery responses remain generic. Recovery never enables an account. Staff mutations also require the relevant permission.

Serialize account status/role changes against protected writes through account-row locks held to transaction end: protected actions that only read account state acquire `FOR SHARE`; operations that update account state or otherwise require an actor-row update lock (including future actor-row quota bookkeeping) acquire `FOR UPDATE` from the outset. Do not first take a shared lock and later upgrade it. Role/status mutations lock accounts before changing authority. If a write obtains its lock first, it finishes before revocation commits; if revocation commits first, the old session fails. Lock all participating accounts in stable ID order for administrative actor/target changes to avoid cross-account deadlocks. Review writes use account → review work → review row → revisions/reports/votes order. Existing account-email operations retain account → token order. Profile/preferences writes must use narrow updates: a marketing-preference change must not rewrite an earlier snapshot of name/address/contact fields. Staff and self-service profile edits require an expected revision to detect competing edits.

## Staff administration and bootstrap

The Users dashboard supports account search/detail, status updates, session revocation, and permitted profile edits. Its staff-access section appears only for authorized administrators and shows combined roles, grant/revoke controls, and an audit timeline. Moderator accounts receive navigation to review moderation.

Bootstrap the first administrator through an operator-only command targeting an existing active account. Grants are idempotent and record an explicit operator identity and reason. Provide a documented, audited operator recovery command for inaccessible administrator accounts; normal routes have no last-admin bypass. Do not promote the first public signup automatically. Preserve at least one active account with the combined `staff.manage` and `users.manage` capabilities (Administrator plus User support in the initial role mapping). Do not disable that last capable account or revoke either required role. An Administrator-only account cannot restore disabled staff through the ordinary UI, so counting the Administrator role alone is insufficient. Use the same transaction-held administration lock for bootstrap/recovery, every role grant/revocation, and every administrative account-status change, acquired before account locks. Do not choose whether to lock from an unlocked target-role lookup. Acquire distinct account IDs in stable order with their strongest required lock modes, then recheck actor authority, target classification, and the combined-capability administrator invariant. Concurrent staff actions must not each remove the last remaining administrator.

Profile name, phone, and address updates must not rewrite order snapshots. Login-email changes continue through the existing ownership/verification workflow; support cannot silently replace a customer's login identity. Never expose or directly edit password hashes or secrets.

## Audit

Write audit events in the same transaction as each status, role, or administrative profile change: actor, target, action, reason, safe before/after metadata, and timestamp. Include an explicit system/operator actor for bootstrap events. Never record passwords, tokens, provider credentials, or unnecessary sensitive field contents. Preserve the existing review/revision-specific decision history; link it to the general audit where useful. Explicit session revocations are audited too.

Use typed/allowlisted metadata: action, changed field names, role/status deltas, stable actor/target IDs, reason, timestamp, and an operator identity when relevant. Do not serialize whole User objects or address snapshots by default. General audit events must survive account removal: use durable identity snapshots and nullable references without cascading audit deletion. `staff.manage` authorizes staff-access audit; `users.manage` plus the staff-target rule authorizes profile/account audit. Moderator-only access is limited to review decisions/reports.

Retain minimal general administrative audit metadata until an explicit retention/purge policy is configured. Do not silently inherit review-table deletion behavior. Any future purge policy must define access, retention interval, and deletion semantics before enabling purges.

## Review migration

Cutover is a coordinated release: run transactional schema/backfill, replace the application and administrative commands together, and exclude all old `review_moderators` writers before retiring that authority. This review feature is not yet deployed, so mixed-version compatibility is unnecessary unless deployment requirements change. A rolling release with old writers is not assumed atomic.

1. Introduce the shared account status, role authority, authorization helper, and general audit.
2. Copy `review_moderators` grants into the Moderator role, preserving current staff access.
3. Change all review page guards and transactional mutation guards to `reviews.moderate` through the shared helper.
4. Make existing moderator grant/revoke CLI aliases call the shared authority and audit changes, or replace them with documented shared commands.
5. Retire the old moderator table only after its reads/writes have been removed and migration tests pass.

User administration supports disable/enable. A database trigger rejects physical account deletion before cascades; existing cascades invert review locking (review → work). Before supporting physical deletion, implement ordered cleanup and deterministic concurrency tests; an AFTER trigger alone is insufficient.

No book, author, ISBN, copy, stock, price, or catalog record changes are required for this work. Do not implement order or refund execution merely by adding a permission; those require authoritative order/payment domain workflows.

## Ownership and implementation sequence

“Plan admin dashboards” owns shared authorization, account status, staff roles, Users/staff-access UI, bootstrap, last-administrator protection, and general audit. “Plan reviews and checkout pause” owns review authorization integration, moderator navigation, and focused review regression tests once the shared helper contract exists. Avoid concurrent edits to shared files; agree on the helper signature before integration.

Required proof: ordinary signup cannot access staff routes; support-only accounts cannot administer staff targets; ordinary routes cannot change the actor’s own roles; each role reaches only its assigned domains; users cannot grant their own roles; disabled and revoked sessions fail; role/status changes and audit commit atomically; concurrent last-admin removals preserve an administrator; migrated moderators retain intended access; published review ratings remain correct; catalog fingerprints remain unchanged.

Additional acceptance gates from Astra: deterministic blocked-transaction tests for revocation-before-write and write-before-revocation across review/account/email paths; concurrent actor/target reversals and last-admin mutations without deadlocks; idempotent bootstrap/recovery; audit rollback/access/retention; obsolete CLI exclusion; title/author tab/newline normalization; and two-cache-instance convergence tests. Existing defects in the architecture review must be corrected and verified before claiming these guarantees.

## Operator setup and local verification

Create the intended staff account through normal signup first. With operator access to the intended database, run:

```bash
cargo run --locked -- --bootstrap-admin existing@example.com operator-name "Initial administrator setup"
# Audited operator recovery uses an existing active account:
cargo run --locked -- --recover-admin existing@example.com operator-name "Administrator recovery"
```

These commands run pending migrations. Bootstrap/recovery grant Administrator plus User support. Staff then use normal `/login`; Account links to User administration (`/staff/users`) and, when assigned Moderator, Review moderation (`/staff/reviews`). Grant and revoke roles from account detail. Security changes require affected users to sign in again.

Local regression and browser coverage includes session revocation interleavings, role boundaries, concurrent administrator invariants, stale profile forms, audit rollback and independent event permissions, moderator migration, review lifecycle, two-process-cache convergence, no-JavaScript forms, mobile layout, and unchanged catalog fingerprints. No hosted database migration or deployment has been performed. Fulfillment/refund permissions reserve future domains; they do not implement order or refund execution.

Verification commands: `cargo fmt --all --check`, `cargo check --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and serial local PostgreSQL tests: 75 passed plus the optional outbox test explicitly passed separately. `scripts/test_admin_browser.cjs` passed on the final build. Run browser suites with fresh local preview processes or separate their bursts: back-to-back automation can intentionally exhaust the existing 120 catalog refreshes/minute admission limit.

Final review browser suite and packaged-route smoke checks also passed on the final build. Review/browser fixtures were confined to the isolated localhost database; catalog fingerprints matched before and after both suites.
