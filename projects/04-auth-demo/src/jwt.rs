//! JWT (HS256 for the capstone demo).
//!
//! The lessons describe RS256 with a published JWKS — production-grade. For the
//! self-contained demo we use HS256 with a single secret because (a) it keeps the
//! project boot-time small (no key generation, no `openssl genpkey`); (b) the
//! patterns (claims, leeway, kid, reuse detection) are identical; (c) flipping
//! to RS256 is a one-day exercise (see EXERCISES.md E6.5).

use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::TryRngCore;
use serde::{Deserialize, Serialize};

pub const ACCESS_TTL_SECS: i64 = 15 * 60;
pub const REFRESH_TTL_SECS: i64 = 30 * 24 * 60 * 60;

const ISSUER: &str = "auth-demo";
const AUDIENCE: &str = "auth-demo-api";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub iat: i64,
    pub nbf: i64,
    pub exp: i64,
    pub jti: String,
    /// "access" | "refresh"
    pub purpose: String,
}

pub struct Jwt {
    enc: EncodingKey,
    dec: DecodingKey,
}

impl Jwt {
    /// Build a JWT instance from a shared secret. The secret should be 32+ bytes.
    pub fn new(secret: &[u8]) -> Self {
        Self {
            enc: EncodingKey::from_secret(secret),
            dec: DecodingKey::from_secret(secret),
        }
    }

    /// Generate a fresh random secret. Useful at startup if no env var is set.
    pub fn random_secret() -> [u8; 32] {
        let mut buf = [0u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut buf)
            .expect("OS RNG must work");
        buf
    }

    pub fn issue_access(&self, user_id: i64) -> Result<String, jsonwebtoken::errors::Error> {
        self.issue(user_id, "access", ACCESS_TTL_SECS)
    }

    pub fn issue_refresh(&self, user_id: i64) -> Result<String, jsonwebtoken::errors::Error> {
        self.issue(user_id, "refresh", REFRESH_TTL_SECS)
    }

    fn issue(
        &self,
        user_id: i64,
        purpose: &str,
        ttl_secs: i64,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            sub: user_id.to_string(),
            iat: now,
            nbf: now,
            exp: now + ttl_secs,
            jti: jti(),
            purpose: purpose.into(),
        };
        let header = Header::new(Algorithm::HS256);
        encode(&header, &claims, &self.enc)
    }

    pub fn verify_access(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let c = self.verify(token)?;
        if c.purpose != "access" {
            return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
        }
        Ok(c)
    }

    pub fn verify_refresh(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let c = self.verify(token)?;
        if c.purpose != "refresh" {
            return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
        }
        Ok(c)
    }

    fn verify(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let mut v = Validation::new(Algorithm::HS256);
        v.set_audience(&[AUDIENCE]);
        v.set_issuer(&[ISSUER]);
        v.leeway = 60;
        decode::<Claims>(token, &self.dec, &v).map(|d| d.claims)
    }
}

fn jti() -> String {
    let mut buf = [0u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_round_trip() {
        let jwt = Jwt::new(&Jwt::random_secret());
        let t = jwt.issue_access(42).unwrap();
        let claims = jwt.verify_access(&t).unwrap();
        assert_eq!(claims.sub, "42");
        assert_eq!(claims.purpose, "access");
    }

    #[test]
    fn refresh_round_trip() {
        let jwt = Jwt::new(&Jwt::random_secret());
        let t = jwt.issue_refresh(42).unwrap();
        let claims = jwt.verify_refresh(&t).unwrap();
        assert_eq!(claims.purpose, "refresh");
    }

    #[test]
    fn access_token_rejected_as_refresh() {
        let jwt = Jwt::new(&Jwt::random_secret());
        let t = jwt.issue_access(42).unwrap();
        assert!(jwt.verify_refresh(&t).is_err());
    }

    #[test]
    fn tampered_signature_rejected() {
        let a = Jwt::new(&Jwt::random_secret());
        let b = Jwt::new(&Jwt::random_secret());
        let t = a.issue_access(42).unwrap();
        assert!(b.verify_access(&t).is_err());
    }
}
