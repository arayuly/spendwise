CREATE TABLE auth_action_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('verify_email', 'reset_password')),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at)
);
CREATE UNIQUE INDEX auth_action_tokens_live_idx ON auth_action_tokens(user_id,purpose) WHERE consumed_at IS NULL;
CREATE INDEX auth_action_tokens_expiry_idx ON auth_action_tokens(expires_at);
