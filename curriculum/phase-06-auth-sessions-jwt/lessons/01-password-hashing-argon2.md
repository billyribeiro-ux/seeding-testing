# Lesson 6.1 — Password Hashing with Argon2id

> **Concept first:** a password database is *the* highest-value target an attacker can hit. The job of hashing is to make a leaked password file useless. Argon2id is the 2026 default.
> **Time:** 25 minutes.

## What hashing buys you

When a user signs up, you receive their cleartext password. You *do not* store it. Not in plain text, not encrypted, not "reversibly encoded." You store a **one-way hash**.

When the same user logs in, you compute the hash of the password they typed and compare it byte-for-byte against the stored hash. If they match, the password is correct.

The hash function must be:

- **Slow.** Fast hashing helps an attacker who steals the database. Argon2id takes ~50–200 ms per hash with sensible parameters — irritating to brute-force, invisible to a logged-in user.
- **Memory-hard.** Modern attackers use GPUs; memory-hard functions resist GPU acceleration.
- **Salted.** A random salt per user means precomputed rainbow tables don't work.

Argon2id is all three. It won the Password Hashing Competition in 2015 and is the OWASP recommendation as of 2026.

## Why not bcrypt, scrypt, PBKDF2?

| Function | When | Why not now |
|---|---|---|
| **bcrypt** | Still safe for legacy code; widely supported | Slower than Argon2id and not memory-hard; max 72 byte input |
| **scrypt** | Memory-hard, mature | Argon2id has better resistance to side-channel attacks |
| **PBKDF2** | FIPS-required environments | Not memory-hard; needs huge iteration counts to keep up |

For *new* systems in 2026: **Argon2id**. For migrating an existing bcrypt store, re-hash on next successful login.

## The `argon2` crate

```toml
[dependencies]
argon2 = "0.5"
rand   = "0.9"
```

```rust
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, PasswordHash, rand_core::OsRng};

fn hash_password(plain: &str) -> argon2::password_hash::Result<String> {
    let salt = SaltString::generate(&mut OsRng);            // 16-byte cryptographic random
    let argon2 = Argon2::default();                          // Argon2id with sane params
    let hash = argon2.hash_password(plain.as_bytes(), &salt)?;
    Ok(hash.to_string())   // PHC string: $argon2id$v=19$m=...,t=...,p=...$salt$hash
}

fn verify_password(plain: &str, stored: &str) -> bool {
    let parsed = PasswordHash::new(stored).expect("stored hash must be PHC-formatted");
    Argon2::default().verify_password(plain.as_bytes(), &parsed).is_ok()
}
```

The stored hash is a single PHC-formatted string that *includes the parameters used*. Years from now you can bump the parameters and old hashes still verify correctly because they remember their own parameters.

## Parameters (knobs and 2026 defaults)

| Parameter | What it controls | OWASP 2026 recommendation |
|---|---|---|
| `m_cost` | Memory used (KiB) | ≥ 19 456 (19 MB); prefer 65 536 (64 MB) where you can afford it |
| `t_cost` | Iterations | ≥ 2 (3 with the 19 MB profile) |
| `p_cost` | Parallelism | 1 (most servers don't benefit from > 1) |
| salt length | | 16 bytes |
| output length | | 32 bytes |

OWASP publishes several equally-acceptable Argon2id profiles; the two most common are **m=19 MB, t=2, p=1** and **m=64 MB, t=3, p=1**. `Argon2::default()` in the `argon2` crate produces the first of these (`m_cost = 19456`, `t_cost = 2`, `p_cost = 1`, 32-byte output) — a secure baseline, *not* the 64 MB profile. If you want the heavier profile, build `Params` explicitly:

```rust
use argon2::{Argon2, Params, Version, Algorithm};
let params = Params::new(65_536, 3, 1, None).expect("valid params");
let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
```

Measure your server: aim for **~100 ms per hash** under load. Faster than that is too easy for attackers; slower than that hurts UX.

## Re-hashing on login (parameter upgrades)

The moment you raise `m_cost` from 64 MB to 128 MB, *new* hashes use the new params. Existing users still have hashes with the old params — and that's fine, they still verify. But you can transparently upgrade them on next login:

```rust
fn login(plain: &str, stored: &str) -> Result<Option<String>, AuthError> {
    let parsed = PasswordHash::new(stored)?;
    Argon2::default().verify_password(plain.as_bytes(), &parsed)?;     // verify first

    // The hash uses parameters from `parsed.params`. If those are weaker than
    // current defaults, compute a new hash and ask the caller to UPDATE the row.
    let current = Argon2::default();
    if parsed.params != current.params() {
        let salt = SaltString::generate(&mut OsRng);
        let new = current.hash_password(plain.as_bytes(), &salt)?;
        return Ok(Some(new.to_string()));    // caller should UPDATE
    }
    Ok(None)
}
```

Users get a free security upgrade on next login. No mass re-hash job, no migration window.

## What to *not* do

- **Don't roll your own KDF.** Cryptography is a minefield. Use the well-vetted crate.
- **Don't pepper.** A "pepper" is a global secret added to every hash. It sounds nice; it adds operational risk (lose the pepper = every login breaks) and the security benefit is marginal vs argon2 with good params.
- **Don't truncate or normalize passwords** unless the spec requires it. Trimming whitespace is fine. Lowercasing is not.
- **Don't log password attempts.** Even on failure. Especially on failure.

## Constant-time comparison

`verify_password` performs constant-time comparison internally — same time regardless of where the mismatch happens. Don't open-code your own comparison; that's where timing attacks live.

## Why this matters

- **A leaked DB with argon2id hashes is *not the end of the world*.** A 12-character random password takes years to crack with modern GPUs at sensible parameters. A weak password is still crackable, but you've made the attacker's life hard.
- **Parameter agility is a security investment.** When NIST publishes new guidance in 2028, you raise the cost factor and everyone re-hashes on next login. No downtime.
- **The PHC format is portable.** Move from one Argon2 library (or language) to another and the hashes still verify.

## Green-bar checkpoint

- You can hash and verify a password using the `argon2` crate.
- You can articulate why re-hashing on login is better than a one-shot migration.
- You can name three things to *not* do (own KDF, pepper, log passwords).

Next: `lessons/02-session-cookies.md`.
