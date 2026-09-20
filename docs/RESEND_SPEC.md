# Resend transactional email specification

Status: implemented locally; production configuration and inbox-delivery release gates remain unverified. Last checked: 2026-09-20.

Implementation and operator details: [Email operations](EMAIL_OPERATIONS.md). The sections below describe the target contract; this document does not claim every release gate is complete.

## Purpose and current state

Provide reliable transactional email for Chantel’s Corner: address verification, password recovery, and security notifications. [Email verification and password recovery](EMAIL_VERIFICATION_SPEC.md) builds on this delivery service.

The Rust/Axum application implements email/password accounts with Argon2 hashes and PostgreSQL sessions. It now includes a Resend HTTP adapter, encrypted PostgreSQL outbox, capture transport, signed webhooks, and account email templates. Hosted sender configuration has not been audited and no live email was sent during implementation. This specification does not claim inbox delivery is working.

Keep Axum, Askama, SQLx, and PostgreSQL. No frontend framework migration, external identity provider, marketing platform, or inbound mailbox is required. Resend sends application email; it does not implement account authentication.

## Provider and domain setup

1. The account owner creates an account at https://resend.com/signup and enables account MFA where available.
2. Add the proposed sending subdomain `notify.chantelscorner.com` in Resend. This is a proposed sender, not a currently verified domain.
3. Add the exact DNS records Resend provides at the domain’s DNS provider. Verify SPF and DKIM. Configure DMARC with a monitored reporting destination; start in monitoring mode and tighten policy after confirming alignment. Preserve existing website, mailbox, MX, and SPF records; never add conflicting SPF records to the same DNS name.
4. Proposed From address: `Chantel’s Corner <accounts@notify.chantelscorner.com>`. Set Reply-To only to an existing monitored support mailbox. Sending through Resend does not create that mailbox.
5. Create a domain-scoped sending API key with the minimum available permissions. Keep it in Railway secrets and an ignored local environment file for controlled development. Never put keys in code, committed files, screenshots, logs, or chat.
6. Disable click and open tracking on authentication messages. Token URLs must not be rewritten for analytics.
7. Configure a signed webhook at `https://www.chantelscorner.com/webhooks/resend` once the endpoint is implemented and deployed. Store its signing secret separately.
8. Send an explicitly authorized test message to an owner-controlled inbox and check authentication headers, content, links, delivery event, and spam placement. Provider acceptance alone is not proof of inbox delivery.

Use `https://www.chantelscorner.com` for all application links. Never derive link origins from request Host or forwarded headers. Local testing may explicitly configure a loopback origin, but it must never appear in production mail.

No paid subscription or DNS change is implied by this document. The free plan currently has a 100-email daily limit; verify current monthly allowances, rates, and account limits before enabling public traffic. Do not silently upgrade a plan.

## Configuration contract

These variables are consumed by the email service:

| Variable | Purpose |
| --- | --- |
| `EMAIL_PROVIDER` | `disabled`, `capture`, or `resend`; production requires `resend` for enabled account-email features |
| `RESEND_API_KEY` | Secret restricted sending credential |
| `EMAIL_FROM` | Validated sender on the verified sending domain |
| `EMAIL_REPLY_TO` | Optional existing support mailbox |
| `PUBLIC_BASE_URL` | Exact trusted application origin; HTTPS mandatory in production |
| `RESEND_WEBHOOK_SECRET` | Secret for webhook signature verification |
| `EMAIL_OUTBOX_KEY` | Random 32-byte encryption key, encoded as base64, for sensitive queued payloads |
| `EMAIL_OUTBOX_KEY_ID` | Key version used to support deliberate rotation and decryption of pending jobs |
| `EMAIL_SENDS_PER_DAY` | Application quota below the provider/account limit |

Configuration is typed and validated at startup. Never silently fall back from Resend to a log-only sender. `capture` is allowed only outside production, writes only to a protected local test sink, and must not leak tokens to shared logs. `disabled` keeps browsing operational but explicitly marks email workflows unavailable rather than reporting that email was sent.

## Module boundaries

- `src/email/`: typed configuration, transport interface, Resend adapter, message rendering, outbox worker, webhook verification.
- Account services own token issuance, verification, expiration, and revocation. The transport never grants account access.
- SQLx migrations own outbox, delivery event, and suppression tables.
- HTML and plain-text email templates live together under `templates/email/`; escape all user-provided text. Subject, sender, destination, and template kind are validated server-side.
- Use one narrowly scoped maintained HTTP client or Resend Rust SDK after checking its maintenance and supported API. Do not add both without a concrete reason.
- Call Resend over HTTPS from the server. Browser code never receives provider credentials or sends provider requests.

## Durable delivery

Issue the account token and queue its message in the same database transaction. A committed account action cannot lose its message because the process restarts before an in-memory send executes.

Proposed `email_outbox` fields: UUID id, message kind, recipient, encrypted immutable payload, encryption key id, status, attempts, next attempt time, lease owner/expiry, provider email id, sanitized error category, created/accepted/expiry timestamps. No raw token or full reset URL may be stored in plaintext, including the outbox. Use authenticated encryption from a maintained library with a fresh nonce for each payload and the job id as associated data. Tokens are separately stored only as hashes by the account service.

Workers claim bounded batches using `FOR UPDATE SKIP LOCKED`, commit the lease, then perform network calls outside the transaction. Expired leases are recoverable. A single process worker is sufficient initially; leases permit multiple replicas without duplicate ownership.

Use the outbox UUID as the Resend idempotency key. The exact payload stays unchanged across retries. Resend currently retains idempotency keys for 24 hours; expire delivery jobs after one hour or token expiry, whichever comes first (verification tokens may remain valid for 24 hours after issuance). Do not automatically replay an ambiguous request beyond that window. Exactly-once delivery is not promised.

Retry timeouts, connection failures, 429 responses, and transient 5xx failures with capped exponential backoff and jitter; honor Retry-After. Treat invalid credentials, invalid sender, and other permanent validation failures as actionable operational errors. Bound attempts and elapsed time. Never retry a token message after its token expires, is revoked, or no longer matches the account email.

Before sending, confirm the recipient is not suppressed and the token is still current. Maintain a shared database quota and provider-rate budget across replicas. Reserve capacity for recovery/security messages. Do not let signup spam exhaust the daily allowance. Quota exhaustion must be observable and must not create permanently stuck jobs.

On success, store the provider email id and `accepted` state. Track delivered, delayed, bounced, complained, and failed separately through provider events. Erase sensitive payload ciphertext after acceptance or terminal failure; retain minimal operational metadata for 30 days by default. Expired token hashes and abandoned jobs are cleaned up on a scheduled bounded maintenance pass.

## Webhooks and operations

Verify Resend/Svix signatures over the original request bytes with timestamp tolerance and the configured secret, before parsing or writing events. Set a small body-size limit. Deduplicate by provider event id. Accept valid events durably before returning success so provider retries are safe. Handle out-of-order events without turning a complaint or permanent bounce back into a healthy recipient.

Persist hard-bounce and complaint suppression for the exact recipient. Never automatically clear it on another send request. Record soft failures separately. Webhook delivery does not verify a user’s email address; only consumption of a valid verification challenge does.

Log job ids, message kinds, attempt counts, sanitized provider status, and elapsed time. Never log bodies, tokens, full URLs, keys, passwords, or raw provider responses. Limit access to recipient data. Monitor queue age, terminal failures, rate-limit responses, daily quota, and webhook failures. Provide a documented operator procedure for inspecting failed jobs without exposing payloads.

## Verification and release gates

1. Unit tests: configuration validation, production capture rejection, template escaping, trusted origin validation, encryption tamper detection, and secret-redaction behavior.
2. Local integration: fake HTTP provider covers acceptance, timeout after acceptance, retry/idempotency, 429, 5xx, permanent errors, crash/lease recovery, multiple workers, quota exhaustion, and expired-job cancellation.
3. Webhooks: valid, tampered, stale, duplicate, unknown, and out-of-order events; suppression prevents future sends.
4. Database tests prove token issuance and queue creation commit or roll back together and raw tokens are absent from database fields.
5. Provider test uses an explicitly authorized owner inbox; verify SPF/DKIM/DMARC alignment and actual received mail.
6. Deploy only after keys, sender DNS, canonical origin, monitoring, and rollback are ready. Keep account-email features disabled until these gates pass. A docs-only change passes none of the runtime gates by itself.

## Sources

- [Resend send API](https://resend.com/docs/api-reference/emails/send-email)
- [Verified domains](https://resend.com/docs/dashboard/domains/introduction)
- [Idempotency keys and retention](https://resend.com/docs/dashboard/emails/idempotency-keys)
- [Webhook verification](https://resend.com/docs/webhooks/verify-webhooks-requests)
- [Current pricing and limits](https://resend.com/pricing)
