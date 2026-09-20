ALTER TABLE users ADD COLUMN email_verified_at timestamptz;
ALTER TABLE users ADD COLUMN auth_version bigint NOT NULL DEFAULT 0 CHECK (auth_version >= 0);
CREATE TABLE account_tokens (
 id uuid PRIMARY KEY, user_id text NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 purpose text NOT NULL CHECK(purpose IN ('verify_email','reset_password','change_email')),
 token_hash text NOT NULL UNIQUE, target_email text NOT NULL, auth_version bigint NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL,
 consumed_at timestamptz, revoked_at timestamptz
);
CREATE INDEX account_tokens_user_purpose ON account_tokens(user_id,purpose);
CREATE TABLE account_email_rate_limits (
 bucket text PRIMARY KEY, attempts integer NOT NULL, expires_at timestamptz NOT NULL
);
