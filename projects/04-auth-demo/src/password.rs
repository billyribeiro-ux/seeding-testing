//! Argon2id password hashing.
//!
//! `hash` produces a PHC-formatted string that includes the parameters used,
//! so `verify` works forever even after we bump parameters in the future.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, password_hash::rand_core::OsRng};

/// A fixed sentinel hash used to keep the login response time constant when the
/// user doesn't exist: the unknown-email path verifies against this so it does
/// the *same* argon2 work as a real verify. Its parameters MUST therefore match
/// what [`hash`] produces (`Argon2::default()` → m=19456, t=2, p=1); otherwise
/// the unknown-email path would take measurably different time and re-open the
/// enumeration timing channel it exists to close.
pub static SENTINEL_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$ZGV2c2VudGluZWxzYWx0$stKOA/bBqGK3fX6EOojIN+BC+sT8SFit385JtMsjgqc";

pub fn hash(plain: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(plain.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify(plain: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(verify("correct horse battery staple", &h));
        assert!(!verify("wrong password", &h));
    }

    #[test]
    fn rejects_garbage_hash() {
        assert!(!verify("anything", "not a real argon2 hash"));
    }

    #[test]
    fn sentinel_parses_and_matches_default_params() {
        // The sentinel must be a valid PHC string (so the unknown-email path
        // actually runs argon2) and its parameters must equal what `hash`
        // produces — otherwise the constant-time defense is defeated by a
        // parameter-driven timing difference between the two paths.
        let parsed = PasswordHash::new(SENTINEL_HASH).expect("sentinel must be valid PHC");
        let from_hash = hash("whatever").unwrap();
        let real = PasswordHash::new(&from_hash).unwrap();
        assert_eq!(
            parsed.params, real.params,
            "sentinel params must match Argon2::default() so timing stays constant"
        );
        // A user-supplied password must not verify against the sentinel.
        assert!(!verify("correct horse battery staple", SENTINEL_HASH));
    }

    #[test]
    fn different_salts_produce_different_hashes() {
        let a = hash("same password").unwrap();
        let b = hash("same password").unwrap();
        assert_ne!(a, b, "salts must randomize the hash");
        assert!(verify("same password", &a));
        assert!(verify("same password", &b));
    }
}
