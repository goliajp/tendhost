use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::Rng;
use tendhost_kernel::{Error, Result};

pub(crate) fn hash(password: &str) -> Result<String> {
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    let salt = SaltString::encode_b64(&salt).map_err(Error::internal)?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(Error::internal)?;
    Ok(hash.to_string())
}

pub(crate) fn verify(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hash_verifies_its_password_only() {
        let h = hash("correct horse").unwrap();
        assert!(verify("correct horse", &h));
        assert!(!verify("correct horsf", &h));
        assert!(!h.contains("correct horse"));
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        assert_ne!(
            hash("correct horse").unwrap(),
            hash("correct horse").unwrap()
        );
    }

    #[test]
    fn a_malformed_hash_verifies_nothing() {
        assert!(!verify("x", "not a hash"));
    }
}
