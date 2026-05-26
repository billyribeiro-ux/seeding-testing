//! Phase 6 stretch — RS256 JWT signing with a published JWKS (E6.5).
//!
//! HS256 is fine for a self-contained service that signs AND verifies
//! its own tokens. The moment a *second* service needs to verify the
//! same JWTs without sharing the symmetric secret, you have to switch
//! to an asymmetric algorithm — RSA-PKCS1-v1_5 with SHA-256 (RS256) is
//! the lingua franca. The signer holds the private key; everyone else
//! reads the public key from `/.well-known/jwks.json` and verifies
//! against it.
//!
//! This module:
//!
//! * Generates a fresh 2048-bit RSA keypair (or loads PEM from env).
//! * Issues access / refresh JWTs with `Algorithm::RS256` and a `kid`
//!   header so downstream verifiers can pick the right key when more
//!   than one is active during a rotation.
//! * Verifies tokens using the matching public key.
//! * Exposes [`JwtRs256::jwks`], which returns the JSON document the
//!   `/.well-known/jwks.json` route serves.
//!
//! Production never generates RSA keys at boot — the key would change
//! every redeploy and invalidate every outstanding token. The pattern
//! is: a long-lived private key mounted from secrets, rotated on a
//! schedule with both the old and new `kid` in the JWKS during the
//! overlap window. We model that overlap by accepting a `&[Jwk]` from
//! callers, but ship a single-key default for the curriculum.

use std::sync::Arc;

use base64::Engine;
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::TryRngCore;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::rand_core::OsRng as RsaOsRng;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};

use crate::jwt::{ACCESS_TTL_SECS, Claims, REFRESH_TTL_SECS};

const ISSUER: &str = "auth-demo";
const AUDIENCE: &str = "auth-demo-api";

/// One JSON Web Key — the public half of an RSA key in the encoding
/// `/.well-known/jwks.json` expects. `n` and `e` are base64url-encoded
/// big-endian bytes (no padding). Stable serde shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
    /// Key type. Always `"RSA"` for this curriculum.
    pub kty: String,
    /// Algorithm the holder of this key signs with. `"RS256"`.
    pub alg: String,
    /// Intended use. `"sig"` for signing.
    #[serde(rename = "use")]
    pub use_: String,
    /// Key id — must match the JWT header's `kid`.
    pub kid: String,
    /// RSA modulus, base64url-encoded big-endian, no padding.
    pub n: String,
    /// RSA public exponent, base64url-encoded big-endian, no padding.
    pub e: String,
}

/// The JWKS document `/.well-known/jwks.json` serves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

/// The HS256 [`crate::jwt::Jwt`]'s RS256 counterpart. Holds one active
/// signing keypair plus the encoded `EncodingKey` / `DecodingKey` that
/// `jsonwebtoken` wants for the hot path.
#[derive(Clone)]
pub struct JwtRs256 {
    inner: Arc<JwtRs256Inner>,
}

struct JwtRs256Inner {
    private_pem: String,
    public_pem: String,
    kid: String,
    enc: EncodingKey,
    dec: DecodingKey,
    // Pre-computed JWK so callers don't reparse on every JWKS request.
    jwk: Jwk,
}

impl JwtRs256 {
    /// Generate a fresh 2048-bit RSA keypair. Returns an error if the
    /// underlying OS RNG fails (extremely rare on real hardware).
    ///
    /// 2048 bits is the OWASP minimum for RSA in 2026. Bumping to 4096
    /// costs ~5× sign time but doubles attacker work — a reasonable
    /// trade for high-value systems.
    pub fn generate() -> Result<Self, JwtRs256Error> {
        let mut rng = RsaOsRng;
        let private =
            RsaPrivateKey::new(&mut rng, 2048).map_err(|e| JwtRs256Error::Keygen(e.to_string()))?;
        let public = RsaPublicKey::from(&private);
        let private_pem = private
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| JwtRs256Error::Keygen(e.to_string()))?
            .to_string();
        let public_pem = public
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| JwtRs256Error::Keygen(e.to_string()))?;
        Self::from_pems(&private_pem, &public_pem)
    }

    /// Load a keypair from PKCS#8 PEM blobs. Production passes both
    /// from secrets-mounted env vars.
    pub fn from_pems(private_pem: &str, public_pem: &str) -> Result<Self, JwtRs256Error> {
        let enc = EncodingKey::from_rsa_pem(private_pem.as_bytes())
            .map_err(|e| JwtRs256Error::Pem(e.to_string()))?;
        let dec = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| JwtRs256Error::Pem(e.to_string()))?;

        // Derive the JWK (n, e) from the public PEM. We parse the
        // public key with the `rsa` crate to extract the BigUint parts.
        let pub_for_jwk = RsaPublicKey::from_pkcs1_pem(public_pem)
            .or_else(|_| {
                // PKCS#8 ("BEGIN PUBLIC KEY") wrapper — extract via SPKI.
                use rsa::pkcs8::DecodePublicKey;
                RsaPublicKey::from_public_key_pem(public_pem)
            })
            .map_err(|e| JwtRs256Error::Pem(e.to_string()))?;

        let n_bytes = pub_for_jwk.n().to_bytes_be();
        let e_bytes = pub_for_jwk.e().to_bytes_be();
        let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&n_bytes);
        let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&e_bytes);
        let kid = kid_from_n_bytes(&n_bytes);

        let jwk = Jwk {
            kty: "RSA".into(),
            alg: "RS256".into(),
            use_: "sig".into(),
            kid: kid.clone(),
            n,
            e,
        };

        Ok(Self {
            inner: Arc::new(JwtRs256Inner {
                private_pem: private_pem.to_string(),
                public_pem: public_pem.to_string(),
                kid,
                enc,
                dec,
                jwk,
            }),
        })
    }

    #[must_use]
    pub fn kid(&self) -> &str {
        &self.inner.kid
    }

    /// The PEM-encoded private key. Treat as a secret — never log it,
    /// never serialize it. Exposed so a deploy script can stash it
    /// behind a secrets manager between restarts.
    #[must_use]
    pub fn private_pem(&self) -> &str {
        &self.inner.private_pem
    }

    /// The PEM-encoded public key. Safe to publish.
    #[must_use]
    pub fn public_pem(&self) -> &str {
        &self.inner.public_pem
    }

    /// The JWK form of the current public key.
    #[must_use]
    pub fn jwk(&self) -> Jwk {
        self.inner.jwk.clone()
    }

    /// The JWKS document — wraps `[jwk]` in `{"keys": [...]}`. This is
    /// the body the `/.well-known/jwks.json` route serves.
    #[must_use]
    pub fn jwks(&self) -> Jwks {
        Jwks {
            keys: vec![self.jwk()],
        }
    }

    pub fn issue_access(&self, user_id: i64) -> Result<String, JwtRs256Error> {
        self.issue(user_id, "access", ACCESS_TTL_SECS)
    }

    pub fn issue_refresh(&self, user_id: i64) -> Result<String, JwtRs256Error> {
        self.issue(user_id, "refresh", REFRESH_TTL_SECS)
    }

    fn issue(&self, user_id: i64, purpose: &str, ttl_secs: i64) -> Result<String, JwtRs256Error> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            sub: user_id.to_string(),
            iat: now,
            nbf: now,
            exp: now + ttl_secs,
            jti: random_jti(),
            purpose: purpose.into(),
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.inner.kid.clone());
        encode(&header, &claims, &self.inner.enc).map_err(|e| JwtRs256Error::Jwt(e.to_string()))
    }

    pub fn verify_access(&self, token: &str) -> Result<Claims, JwtRs256Error> {
        let c = self.verify(token)?;
        if c.purpose != "access" {
            return Err(JwtRs256Error::WrongPurpose);
        }
        Ok(c)
    }

    pub fn verify_refresh(&self, token: &str) -> Result<Claims, JwtRs256Error> {
        let c = self.verify(token)?;
        if c.purpose != "refresh" {
            return Err(JwtRs256Error::WrongPurpose);
        }
        Ok(c)
    }

    fn verify(&self, token: &str) -> Result<Claims, JwtRs256Error> {
        let mut v = Validation::new(Algorithm::RS256);
        v.set_audience(&[AUDIENCE]);
        v.set_issuer(&[ISSUER]);
        v.leeway = 60;
        decode::<Claims>(token, &self.inner.dec, &v)
            .map(|d| d.claims)
            .map_err(|e| JwtRs256Error::Jwt(e.to_string()))
    }
}

/// Errors from RS256 keygen / signing / verification.
#[derive(Debug, thiserror::Error)]
pub enum JwtRs256Error {
    #[error("rsa keygen failed: {0}")]
    Keygen(String),
    #[error("rsa pem load failed: {0}")]
    Pem(String),
    #[error("jwt error: {0}")]
    Jwt(String),
    #[error("wrong token purpose")]
    WrongPurpose,
}

/// Derive a stable `kid` from the modulus. SHA-256 over `n` truncated
/// to 16 bytes, base64url-encoded. Same modulus → same kid across
/// restarts, which matters when a key is reloaded from secrets.
fn kid_from_n_bytes(n_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(n_bytes);
    let digest = h.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&digest[..16])
}

fn random_jti() -> String {
    let mut buf = [0u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keygen is expensive (~hundred ms); share one across tests.
    fn shared_jwt() -> JwtRs256 {
        use std::sync::OnceLock;
        static SHARED: OnceLock<JwtRs256> = OnceLock::new();
        SHARED
            .get_or_init(|| JwtRs256::generate().expect("keygen"))
            .clone()
    }

    #[test]
    fn round_trip_access_token() {
        let jwt = shared_jwt();
        let t = jwt.issue_access(42).unwrap();
        let c = jwt.verify_access(&t).unwrap();
        assert_eq!(c.sub, "42");
        assert_eq!(c.purpose, "access");
    }

    #[test]
    fn refresh_rejected_as_access_and_vice_versa() {
        let jwt = shared_jwt();
        let refresh = jwt.issue_refresh(42).unwrap();
        assert!(jwt.verify_access(&refresh).is_err());
        let access = jwt.issue_access(42).unwrap();
        assert!(jwt.verify_refresh(&access).is_err());
    }

    #[test]
    fn header_carries_kid() {
        let jwt = shared_jwt();
        let t = jwt.issue_access(42).unwrap();
        let header = jsonwebtoken::decode_header(&t).unwrap();
        assert_eq!(
            header.kid.as_deref(),
            Some(jwt.kid()),
            "JWT header.kid must match the signer's kid so multi-key verifiers can route"
        );
        assert_eq!(header.alg, Algorithm::RS256);
    }

    #[test]
    fn jwks_shape_matches_rfc7517() {
        let jwt = shared_jwt();
        let jwks = jwt.jwks();
        assert_eq!(jwks.keys.len(), 1);
        let k = &jwks.keys[0];
        assert_eq!(k.kty, "RSA");
        assert_eq!(k.alg, "RS256");
        assert_eq!(k.use_, "sig");
        assert!(!k.kid.is_empty());
        // base64url-no-pad alphabet only.
        for (name, v) in [("n", &k.n), ("e", &k.e)] {
            assert!(
                v.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "{name} must be base64url-no-pad: {v}"
            );
        }
    }

    #[test]
    fn token_from_one_keypair_does_not_verify_with_another() {
        let a = shared_jwt();
        let b = JwtRs256::generate().unwrap();
        let t = a.issue_access(42).unwrap();
        assert!(
            b.verify_access(&t).is_err(),
            "RS256 verification with a different public key must fail"
        );
    }
}
