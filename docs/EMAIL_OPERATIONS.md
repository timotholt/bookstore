# Transactional email operations

Production email is enabled through Resend as of 2026-09-20. The verified sending domain is notify.chantelscorner.com. An initial isolated local email incorrectly used local links; Resend mode now requires the canonical public origin even outside production. Release c4ac80e9db4a0980e079a48ecd5b26c543844d1e is deployed successfully. A real Chrome reset request reached Gmail at 12:02 PM Pacific, with one provider attempt and signed email.sent/email.delivered events persisted. The reset button, fallback URL, and bookstore link use https://www.chantelscorner.com. Clicking the email button opened the public reset form and removed the bearer token from the address bar. Actual password replacement awaits user entry; live email verification completion remains pending.

Set `EMAIL_PROVIDER=resend` only after verifying the sender domain and completing an authorized inbox test. Required settings: `RESEND_API_KEY`, `RESEND_WEBHOOK_SECRET` (Svix `whsec_` secret), `EMAIL_FROM`, `EMAIL_OUTBOX_KEY` (base64 random 32 bytes), `EMAIL_OUTBOX_KEY_ID`, and `PUBLIC_BASE_URL=https://www.chantelscorner.com`. Optional: `EMAIL_REPLY_TO`, `EMAIL_SENDS_PER_DAY` (default 90). Production accepts only the canonical origin. Sender display names are supported. Configure tracking disabled in Resend; this application does not enable it.

Development capture requires `EMAIL_PROVIDER=capture`, the same encryption configuration, a valid `EMAIL_FROM`, and explicit `EMAIL_CAPTURE_DIR`. The sink contains real action links; use only local test accounts. The directory is mode 0700 and each JSON message mode 0600. It must not be under a served asset directory or tracked in Git. No fallback from Resend to capture occurs. Capture is forbidden in production.

The worker persists an encrypted complete provider payload when the account transaction commits. UUID idempotency keys and immutable payloads survive retries. A 60-second recoverable lease exceeds the 15-second HTTP timeout. A shared database budget limits attempts to one per second, caps daily attempts, and reserves 20% against verification signup traffic. Jobs expire within one hour or token expiry; currentness and suppression are rechecked before sending. Accepted and terminal jobs lose ciphertext. Minimal outbox/event metadata is retained 30 days; suppression records remain until a deliberate operator decision.

Safe queue inspection (no recipient or payload exposure):

```sql
SELECT status, kind, error_category, count(*), min(created_at)
FROM email_outbox GROUP BY status, kind, error_category;
SELECT day, total, verification FROM email_send_budget ORDER BY day DESC LIMIT 7;
```

`accepted` means provider acceptance, not inbox delivery. Inspect `email_delivery_events` for delivery/bounce/complaint outcomes. Webhooks verify raw-body Svix HMAC/timestamp, deduplicate event IDs transactionally, retain event outcomes independently of arrival order, and persist permanent-bounce/complaint/suppressed recipients. Unknown authenticated event kinds are retained. An event never verifies an account.

For failures: repair sender credentials/configuration, then ask the user to request a fresh link. Do not replay expired jobs or reconstruct erased payloads. Rotate encryption keys only after draining/cancelling pending jobs; unknown key versions fail closed. Automatic multi-key rollover is not implemented. Capture files require manual deletion after development use. Clock synchronization matters for signatures and token expiry.

Local checks:

```sh
cargo test email::tests
EMAIL_TEST_DATABASE_URL=postgres://timotholt@127.0.0.1:55439/postgres cargo test email::tests::outbox_atomic_encryption_lease_and_suppression -- --ignored
```

The isolated-schema integration test verifies rollback, encrypted storage, concurrent worker claims, stale lease recovery, payload erasure, protected capture, and suppression. Fake HTTP tests verify accepted/429/5xx/permanent responses and identical retry payload/idempotency keys. The authorized Gmail reset message reached the inbox and exposed the embedded bookstore image in the email body. Rendering across other clients, full authentication-header inspection, and crash-after-provider-acceptance testing remain unverified.

Real Resend messages embed the bookstore JPEG as a CID attachment so rendering does not depend on a public asset URL. Preview HTML uses a relative image path.

Account form pages use Referrer-Policy: strict-origin so Chrome preserves the Origin required by CSRF validation without sharing URL paths or queries. Token-to-session redirects retain no-referrer. The regression test rejects Origin: null and verifies both policies; native Chrome submissions passed locally and in production.

Reset UX revision (2026-09-20): new passwords use 15–128 Unicode characters; short and long inputs have separate plain-language errors. Validation failures render the reset form again without password values and without consuming the token. Active reset forms have no recovery/verification/account footer; unavailable links show a separate recovery screen. PR #6 is merged and release 673b91628042285636f88925c43de514139a2359 deployed successfully. Focused local account tests, strict clippy, and branch/main CI passed. Native Chrome verified the public reset form displays the new guidance, two password inputs, and one action without the old footer; a separate cookie-free public request verified missing challenges show recovery without password fields. No user password was changed during this UX test.
