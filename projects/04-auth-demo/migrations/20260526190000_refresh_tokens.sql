-- Phase 6 stretch — refresh-token reuse detection (E6.7).
--
-- Every refresh JWT corresponds to a row here, keyed by its `jti` claim.
-- The flow is **rotation with a family**:
--
--   /auth/login:
--     mint a refresh JWT, INSERT a row with parent_jti = NULL and a fresh
--     family_id. This is the root of the family.
--
--   /auth/refresh:
--     atomically `UPDATE … SET used_at = NOW() WHERE jti = ? AND
--     used_at IS NULL AND revoked_at IS NULL RETURNING …`.
--
--     * Row claimed → legitimate use. INSERT the new refresh with
--       parent_jti = old jti and the same family_id.
--     * UPDATE returned no row, but the row exists with used_at ≠ NULL
--       → **REUSE DETECTED**. The legitimate user already burned this
--       token; whoever is presenting it now is replaying. UPDATE
--       revoked_at on every row in the family and audit-log it. The
--       attacker AND the legitimate user are both forced to re-login.
--     * Row missing or already revoked → treat as Unknown and reject.
--
-- That "kill the whole family on reuse" rule is what closes the loop:
-- a stolen refresh token is unusable as soon as either side rotates,
-- because the other side's next call will trip the detector.

CREATE TABLE IF NOT EXISTS refresh_tokens (
    jti         TEXT    PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    family_id   TEXT    NOT NULL,
    parent_jti  TEXT    REFERENCES refresh_tokens(jti) ON DELETE SET NULL,
    issued_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at  TEXT    NOT NULL,
    used_at     TEXT,
    revoked_at  TEXT
);

CREATE INDEX IF NOT EXISTS refresh_tokens_family_idx
    ON refresh_tokens (family_id);

CREATE INDEX IF NOT EXISTS refresh_tokens_user_active_idx
    ON refresh_tokens (user_id) WHERE revoked_at IS NULL;
