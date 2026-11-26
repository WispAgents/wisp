//! Share passwords, kept as v1 kept them (grid/shares.js `hashPasskey`): scrypt N=16384, r=8, p=1, 32 bytes, as
//! `<salt hex>:<hash hex>`, compared in constant time, so a live v1 row still opens.
use std::fmt::Write as _;

use subtle::ConstantTimeEq;

/// A password as kept: `<salt hex>:<hash hex>` (v1's `hashPasskey`: scrypt N=16384, r=8, p=1, 32 bytes).
pub fn hash_password(pass: &str) -> String {
    let salt: [u8; 16] = rand::random();
    format!("{}:{}", hex(&salt), hex(&derive(pass, &salt)))
}

/// Every password `derive` hashed, in tests: a refused guess must cost no scrypt, and only a count can show that.
#[cfg(test)]
pub(crate) static DERIVED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// How many times `derive` hashed `pass` (tests).
#[cfg(test)]
pub(crate) fn derived(pass: &str) -> usize {
    DERIVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().filter(|p| *p == pass).count()
}

fn derive(pass: &str, salt: &[u8]) -> Vec<u8> {
    #[cfg(test)]
    DERIVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(pass.to_owned());
    let mut out = vec![0u8; 32];
    if let Ok(p) = scrypt::Params::new(14, 8, 1, 32) {
        let _ = scrypt::scrypt(pass.as_bytes(), salt, &p, &mut out);
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().fold(String::with_capacity(b.len() * 2), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    s.len()
        .is_multiple_of(2)
        .then(|| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect::<Option<Vec<u8>>>())
        .flatten()
}

/// Whether `pass` could be a share's password at all: 1 to `PASSWORD_MAX` bytes, as `put` takes one. Anything else
/// is wrong without hashing it, so a megabyte "guess" costs the grid nothing.
pub fn could_be_password(pass: &str) -> bool {
    !pass.is_empty() && pass.len() <= super::PASSWORD_MAX
}

/// Whether `pass` is the password `kept`. scrypt: ~50 ms of CPU, so the edge runs it on the blocking pool.
pub fn check_password(kept: &str, pass: &str) -> bool {
    if !could_be_password(pass) {
        return false;
    }
    let Some((salt, hash)) = kept.split_once(':') else { return false };
    let (Some(salt), Some(want)) = (unhex(salt), unhex(hash)) else { return false };
    let got = derive(pass, &salt);
    got.len() == want.len() && bool::from(got.ct_eq(&want))
}
