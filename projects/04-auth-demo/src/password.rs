//! Argon2id password hashing.
//!
//! `hash` produces a PHC-formatted string that includes the parameters used,
//! so `verify` works forever even after we bump parameters in the future.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, password_hash::rand_core::OsRng};

/// A fixed sentinel hash used to keep the login response time constant when the
/// user doesn't exist. Computed once at first use.
pub static SENTINEL_HASH: &str = "$argon2id$v=19$m=65536,t=3,p=4$ZGV2c2VudGluZWxzYWx0$M+pq1Mhgw3qfb8MK1pSwfn9k0NLm6/MhAJjvgMTcUjk";

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
    fn different_salts_produce_different_hashes() {
        let a = hash("same password").unwrap();
        let b = hash("same password").unwrap();
        assert_ne!(a, b, "salts must randomize the hash");
        assert!(verify("same password", &a));
        assert!(verify("same password", &b));
    }
}
