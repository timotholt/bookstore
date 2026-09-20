# Email verification and password recovery specification

Status: proposed; not implemented. Last checked: 2026-09-20.
Dependency: [Resend transactional email specification](RESEND_SPEC.md).

## Outcome and current state

Customers can prove ownership of their email address, resend an expired verification link, and recover a forgotten password using real email delivery. The flows use the existing Rust/Axum, Askama, PostgreSQL, SQLx, Argon2, and tower-sessions stack. Google/OAuth is not required.

The existing app supports signup, email/password login, profile updates, and PostgreSQL sessions. It does not implement verification or password recovery. The account security page’s password link currently leads to profile editing. Existing accounts must not be backfilled as verified without proof. This document specifies future behavior, not deployed functionality.

## Product behavior

Unverified customers may sign in, browse, manage their profile, and use their cart. Show a clear verification status and resend action in account security. Require verified email for future review submission, staff privileges, and future completed checkout. Enforce those rules in domain services, not merely by hiding controls. Current checkout remains a preview.

Do not automatically sign a user in when they follow a verification or reset link. A verification proves ownership of one email address; it is not an OAuth login or MFA. Following a link must not sign out or switch an unrelated account already open in the browser.

If delivery is not configured, show a truthful temporary-unavailability state. Do not create a dead end that claims email was sent. Provider failures after a successful enqueue are handled by the outbox worker, monitored operationally, and recoverable through bounded resends.

## Data model

Add SQLx migrations for:

- `users.email_verified_at` nullable timestamp and `users.auth_version` nonnegative integer, initially 0. All existing users start unverified.
- `account_tokens`: id, user id, purpose (`verify_email`, `reset_password`, `change_email`), SHA-256 digest of the token, exact normalized target email, issued auth version, created/expiry/consumed/revoked timestamps. Unique token digest, user/purpose indexes, and valid-purpose constraints.
- Shared rate-limit buckets with atomic database updates and bounded retention. Use purpose-specific hashes for recipient/IP keys to avoid storing unnecessary raw identifiers.
- The encrypted outbox and delivery tables from the Resend spec.
- Pending email changes hold the new address, creation time, and challenge reference; the login email remains unchanged until confirmation.

Use a cryptographically secure RNG to create at least 32 random token bytes, encoded URL-safe. Store only its SHA-256 hash in the token table. The encrypted outbox is the only durable recoverable copy needed to send/retry a link; follow the Resend spec’s encryption and deletion rules. Never use sequential ids, timestamps, or a plain UUID as the security token.

Verification links expire after 24 hours. Password-reset links expire after 30 minutes. Application timestamps use UTC and expiry is checked against database time. Bind each token to its purpose, user, exact email, and issued auth version.

## Routes and forms

| Route | Behavior |
| --- | --- |
| `GET /account/verification` | Signed-in account’s verification status and resend form |
| `POST /account/verification/resend` | Queue a new verification email, subject to limits |
| `GET /verify-email?token=...` | Establish a short-lived challenge session and redirect to a clean confirmation URL; do not consume token |
| `POST /verify-email` | Explicit confirmation; atomically consume challenge and verify matching email |
| `GET /forgot-password` | Email entry form |
| `POST /forgot-password` | Generic acknowledgement; queue recovery only for an eligible account |
| `GET /reset-password?token=...` | Establish challenge session and redirect to clean reset form; do not consume token |
| `POST /reset-password` | Validate new password, consume token, update hash, revoke sessions, queue security notice |
| `POST /account/email-change` | Reauthenticate and queue confirmation to a proposed new email |
| `GET/POST /confirm-email-change` | Same safe link exchange and explicit confirmation pattern |

Use normal semantic forms, existing Rust UI view objects, Askama includes, and shared CSS families. Add “Forgot password?” to sign-in and correct the account-security password action. Include clear success, expired/invalid, rate-limited, and unavailable states. Preserve keyboard navigation, labels, autocomplete, and accessible errors. Do not put tokens in analytics attributes or hidden page instrumentation.

Token landing responses and all challenge pages use `Cache-Control: no-store` and `Referrer-Policy: no-referrer`; exclude third-party resources and analytics. Configure application, proxy, and hosting access logs to redact token query strings. Link exchanges do not invalidate tokens: email security scanners may fetch GET URLs. On landing, store the challenge in the server-side session, then redirect to a token-free URL. The subsequent POST needs a session-bound CSRF token and same-origin validation. No open redirect parameter is accepted.

## Signup and verification

1. Validate and normalize the address consistently using a maintained validator; reject malformed or overlong input. Do not strip Gmail dots, remove plus tags, or otherwise guess provider aliases.
2. In one transaction create user, password credential, verification token hash, and encrypted email job. Password hashing occurs outside the transaction on a bounded blocking worker.
3. Retain the current signed-in signup experience, but display unverified status. Rotate the session identifier when establishing login.
4. Email identifies Chantel’s Corner, explains the action, gives the expiry and one confirmation link, and tells an unintended recipient to ignore it. Include HTML and plain text.
5. Confirmation locks the user/token, verifies all bindings and expiry, marks email verified, consumes the token, and revokes sibling verification tokens in a single transaction. Replays never change state.
6. A resend is authenticated, CSRF-protected, and rate-limited. Issuing a replacement revokes earlier verification tokens and cancels pending sibling mail jobs. Explain that the newest link should be used. An already verified account does not receive another challenge.

Unverified ownership is not enough to change a password or inherit privileges. Verification never silently upgrades staff roles or links third-party identities.

## Password reset

1. Always respond to an accepted request with: “If an eligible account exists for that address, we’ll email password reset instructions.” Use the same HTTP status, body, and asynchronous request path for existing, missing, suppressed, and account-throttled addresses. Do not expose provider errors that reveal account existence.
2. Apply source/global abuse checks before account lookup. Keep known/unknown response timing comparable through asynchronous delivery and measured tests; a generic message alone is insufficient.
3. Requesting a reset does not lock the account, change its password, revoke sessions, or invalidate otherwise valid reset links. This prevents an attacker from repeatedly requesting links to deny account access. Bound concurrently active reset tokens and resend frequency.
4. After the recipient opens the link, require new password plus confirmation. Reuse a single password policy for signup and reset. Target minimum 15 characters for password-only accounts, maximum at least 64 characters, no silent truncation, no arbitrary composition rules, and a common/breached-password blocklist using a local list or privacy-preserving lookup. Accept spaces, Unicode, and password-manager generated passwords. Existing passwords remain usable until changed.
5. Verify the token before expensive hashing, hash the replacement with Argon2 on a bounded blocking worker, then lock and recheck the token inside the write transaction. Concurrent submissions permit only one success.
6. Atomically replace the hash, increment `auth_version`, consume the token, revoke every outstanding reset token, and queue a password-change notification to the existing account email. Reset does not grant `email_verified_at`; verification remains an explicit separate action.
7. Every authenticated session contains the version from login. Every account lookup checks it against the current database version; mismatch invalidates authentication. This includes existing sessions created before versioning: treat missing version as invalid after rollout. Prevent a concurrent old-password login from adopting the new version by comparing the credential/version snapshot when establishing the session.
8. Clear the reset challenge, rotate/clear relevant session state, and redirect to normal sign-in. Do not auto-login. The user sees that other sessions have been signed out.

Tokens fail uniformly when malformed, expired, wrong-purpose, consumed, revoked, email-mismatched, or version-mismatched. Never log the token while reporting failure. Rate-limit invalid confirmation attempts as well as email issuance.

## Email changes

The current profile endpoint can directly update email. Replace that behavior as part of this work; otherwise verification and recovery can be undermined.

Require a recent current-password check plus CSRF protection to request an email change. Keep the old login address and verification status until the new address is confirmed. Send the new address a purpose-specific challenge and notify the old address of the request. On confirmation, lock the user, recheck uniqueness and auth version, switch to the new address, mark it verified, revoke old tokens, increment auth version, and cancel obsolete outbox messages. Notify the old address that the change completed. Never transfer a verified flag to an unconfirmed address.

For a future federated-only account, define an equivalent reauthentication policy before enabling email changes. Do not implement a password-free bypass now.

## Abuse controls

Initial tunable limits, enforced atomically in PostgreSQL across replicas:

- One issuance per account/purpose per 60 seconds, at most 5 per hour and 10 per day.
- At most 20 email-workflow requests per trusted client IP per hour, plus a global shared daily sending budget below the Resend limit.
- At most 10 invalid challenge submissions per source per 15 minutes; never permanently lock an account on unauthenticated failures.
- Cap request-body and field sizes, worker concurrency, and password-hash jobs.

Use the actual peer address unless a documented trusted reverse proxy supplies client identity. Never trust arbitrary forwarded headers. Test Railway’s forwarding behavior before enabling IP-based production enforcement. Account throttling remains mandatory even when source identity is unavailable. Add risk-triggered CAPTCHA later only if measured abuse warrants it.

Protect all new state-changing browser routes with session-bound CSRF tokens and origin validation. Keep secure, HttpOnly, SameSite cookies; establish explicit session expiry. Review existing signup/profile mutations so they cannot bypass the new controls. Suppress authentication mail to hard-bounced or complained recipients without revealing this state on public forms.

## Acceptance gates

| Area | Required proof |
| --- | --- |
| Verification | Signup queues one message; valid POST verifies exact address; GET/scanner fetch never verifies; replay/expiry/wrong-purpose rejected |
| Recovery | Known/unknown requests have matching outward behavior; old password fails after reset; new password works; no automatic login |
| Concurrency | Two simultaneous reset/verification submissions produce one successful mutation; transaction rollback leaves no orphan job/token |
| Sessions | All older sessions stop authenticating after reset; concurrent login cannot preserve old-password access |
| Email changes | Reauthentication required; old address unchanged before confirmation; uniqueness races and stale links handled safely |
| Abuse | Account/source/global limits work across replicas and restarts; forged forwarded headers cannot evade limits |
| Secrecy | No plaintext tokens in DB/logs/analytics; no query referrer leakage; outbox tampering detected |
| Delivery | Resend test proves actual receipt, valid production link, expiry copy, and HTML/plain-text rendering |
| UX | Sign-in recovery link, account verification state/resend, invalid/expired paths, keyboard and mobile forms checked |
| Regression | `cargo check`, meaningful PostgreSQL-backed `cargo test`, catalog/cart/account smoke checks pass |

Use isolated local database schemas, fake transport, and a controllable clock for automated verification. Never send real customer emails from tests. A final controlled owner-inbox test requires an explicitly identified recipient and authorization.

## Implementation sequence and rollout

1. Implement and test the Resend spec’s transport, encrypted outbox, configuration, and observability.
2. Add token/rate-limit/version migrations and domain services.
3. Integrate signup verification, safe resend/confirmation, password reset, and email-change protection.
4. Add shared UI components and meaningful security/concurrency tests.
5. Provision sending DNS/secrets and confirm end-to-end delivery. Deploy only after acceptance gates pass; record exact build and verification evidence.

Rollout invalidates pre-versioning sessions intentionally and requires customers to sign in again. Keep additive schema changes compatible with rollback, but do not roll back to a binary that ignores auth-version revocation after password resets have been enabled. During incidents, disable issuance/sending independently while preserving token and session enforcement.

## Sources

- [OWASP Forgot Password Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html)
- [OWASP Email Validation and Verification](https://cheatsheetseries.owasp.org/cheatsheets/Email_Validation_and_Verification_Cheat_Sheet.html)
- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
- [Project architecture](PRODUCT_ARCHITECTURE_SPEC.md)
- [Project infrastructure](INFRASTRUCTURE_SPEC.md)
