//! Bearer secrets (session tokens, api tokens, enrollment tokens, host
//! credentials). The server keeps only the fingerprint; the plain value
//! exists once, in the response that hands it out.

use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use rand::Rng;
use sha2::{Digest, Sha256};

/// 256 random bits behind a prefix that names the kind of secret.
pub fn generate(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    format!("{prefix}_{}", BASE64URL_NOPAD.encode(&bytes))
}

/// What is stored and looked up instead of the secret. Unsalted on purpose:
/// the input is 256 random bits, and lookup by fingerprint needs it stable.
pub fn fingerprint(secret: &str) -> String {
    HEXLOWER.encode(Sha256::digest(secret.as_bytes()).as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_and_not_the_secret() {
        let s = generate("ths");
        assert!(s.starts_with("ths_"));
        assert_eq!(fingerprint(&s), fingerprint(&s));
        assert_ne!(fingerprint(&s), fingerprint(&generate("ths")));
        assert!(!fingerprint(&s).contains(&s[4..]));
        assert_eq!(fingerprint(&s).len(), 64);
    }
}
