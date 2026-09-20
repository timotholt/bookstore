CREATE TABLE email_outbox (
 id UUID PRIMARY KEY, kind TEXT NOT NULL, recipient TEXT NOT NULL, token_id UUID,
 payload BYTEA, key_id TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
 attempts INTEGER NOT NULL DEFAULT 0, next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 lease_owner UUID, lease_until TIMESTAMPTZ, provider_id TEXT,
 error_category TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 accepted_at TIMESTAMPTZ, expires_at TIMESTAMPTZ NOT NULL,
 CHECK (status IN ('pending','sending','accepted','cancelled','failed','expired'))
);
CREATE INDEX email_outbox_pending ON email_outbox(next_attempt_at) WHERE status IN ('pending','sending');
CREATE INDEX email_outbox_token ON email_outbox(token_id);
CREATE TABLE email_delivery_events (event_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, kind TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE email_suppressions (recipient TEXT PRIMARY KEY, reason TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE email_send_budget (day DATE PRIMARY KEY, total INTEGER NOT NULL DEFAULT 0, verification INTEGER NOT NULL DEFAULT 0, last_send_at TIMESTAMPTZ);
