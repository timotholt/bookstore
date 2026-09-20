# Transactional email operations

Implementation exists; sending is disabled by default. On 2026-09-20, Resend verified notify.chantelscorner.com and an isolated local test message arrived in Gmail with the embedded bookstore artwork. That initial message used local links and was not a successful production workflow test. Resend mode now requires the canonical public origin even outside production. Live deployment and the complete public-link workflow remain pending.

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

The isolated-schema integration test verifies rollback, encrypted storage, concurrent worker claims, stale lease recovery, payload erasure, protected capture, and suppression. Fake HTTP tests verify accepted/429/5xx/permanent responses and identical retry payload/idempotency keys. Live DNS/header verification, mailbox rendering across clients, crash-after-provider-acceptance, and actual inbox/spam placement remain release gates.

Real Resend messages embed the bookstore JPEG as a CID attachment so rendering does not depend on a public asset URL. Preview HTML uses a relative image path.
