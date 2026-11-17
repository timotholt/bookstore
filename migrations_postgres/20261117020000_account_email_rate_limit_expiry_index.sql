-- Expiry cleanup runs on the request path; keep it an index range scan.
-- Production applies this as the owner via `chantels-corner --migrate`
-- (docs/PRODUCTION_MIGRATIONS_SPEC.md). Indexes need no grants.
CREATE INDEX account_email_rate_limits_expires_at_idx ON account_email_rate_limits (expires_at);
