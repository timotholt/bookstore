-- Expiry cleanup runs on the request path; keep it an index range scan.
CREATE INDEX account_email_rate_limits_expires_at_idx ON account_email_rate_limits (expires_at);
