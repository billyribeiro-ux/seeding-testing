#[test]
fn gen_sentinel() {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::Argon2;
    let salt = SaltString::from_b64("ZGV2c2VudGluZWxzYWx0").unwrap();
    let argon2 = Argon2::default();
    let h = argon2.hash_password(b"auth-demo-sentinel-not-a-real-password", &salt).unwrap();
    println!("SENTINEL={}", h.to_string());
}
