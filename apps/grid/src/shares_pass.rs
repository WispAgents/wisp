//! Share passwords, kept as v1 kept them (grid/shares.js `hashPasskey`): scrypt N=16384, r=8, p=1, 32 bytes, as
//! `<salt hex>:<hash hex>`, compared in constant time, so a live v1 row still opens.
use std::fmt::Write as _;

use subtle::ConstantTimeEq;

/// A password as kept: `<salt hex>:<hash hex>` (v1's `hashPasskey`: scrypt N=16384, r=8, p=1, 32 bytes).
pub fn hash_password(pass: &str) -> String {
    let salt: [u8; 16] = rand::random();
    format!("{}:{}", hex(&salt), hex(&derive(pass, &salt)))
}

fn derive(pass: &str, salt: &[u8]) -> Vec<u8> {
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

pub fn check_password(kept: &str, pass: &str) -> bool {
    let Some((salt, hash)) = kept.split_once(':') else { return false };
    let (Some(salt), Some(want)) = (unhex(salt), unhex(hash)) else { return false };
    if pass.is_empty() {
        return false;
    }
    let got = derive(pass, &salt);
    got.len() == want.len() && bool::from(got.ct_eq(&want))
}
