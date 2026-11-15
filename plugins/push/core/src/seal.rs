//! A note sealed to one phone (HPKE, RFC 9180, base mode: DHKEM(X25519, HKDF-SHA256) / HKDF-SHA256 /
//! ChaCha20-Poly1305, `info` [`HPKE_INFO`], the collapse key as associated data so a relay can't move a note onto
//! another task). `sealed` is base64url of the 32-byte encapsulated key followed by the ciphertext. [`open`] is
//! what the phone does (steps 9/10 port it to Kotlin/Swift); it is here so the tests can open what wisp sent.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64U;
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::rand_core::CryptoRng;
use hpke::{Deserializable, Kem as _, OpModeR, OpModeS, Serializable};
use wisp_push_api::{HPKE_INFO, Note};

use crate::{PushError, Result};

type Kem = X25519HkdfSha256;
const ENC_LEN: usize = 32;

fn crypto(what: &str) -> PushError {
    PushError::Crypto(what.to_owned())
}

/// A phone's key pair, base64url (what the phone keeps in its Keystore/Keychain, and gives wisp the public half of).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    pub secret: String,
    pub public: String,
}

impl Keys {
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let (sk, pk) = Kem::gen_keypair(rng);
        Self { secret: B64U.encode(sk.to_bytes()), public: B64U.encode(pk.to_bytes()) }
    }
}

/// `note` sealed to `public_key` (base64url), bound to `collapse`. → `sealed`.
pub fn seal(note: &Note, public_key: &str, collapse: &str, rng: &mut impl CryptoRng) -> Result<String> {
    let pk_bytes = B64U.decode(public_key.trim_end_matches('=')).map_err(|_| crypto("the phone's key isn't base64url"))?;
    let pk = <Kem as hpke::Kem>::PublicKey::from_bytes(&pk_bytes).map_err(|_| crypto("the phone's key isn't an X25519 key"))?;
    let plain = serde_json::to_vec(note).map_err(|e| crypto(&e.to_string()))?;
    let (enc, ct) =
        hpke::single_shot_seal::<ChaCha20Poly1305, HkdfSha256, Kem, _>(&OpModeS::Base, &pk, HPKE_INFO, &plain, collapse.as_bytes(), rng)
            .map_err(|_| crypto("the note couldn't be sealed"))?;
    let mut out = enc.to_bytes().to_vec();
    out.extend_from_slice(&ct);
    Ok(B64U.encode(out))
}

/// What the phone does: `sealed` opened with its secret key, for the collapse key it came with.
pub fn open(sealed: &str, secret_key: &str, collapse: &str) -> Result<Note> {
    let raw = B64U.decode(sealed).map_err(|_| crypto("not base64url"))?;
    if raw.len() <= ENC_LEN {
        return Err(crypto("too short to be a note"));
    }
    let (enc, ct) = raw.split_at(ENC_LEN);
    let sk_bytes = B64U.decode(secret_key.trim_end_matches('=')).map_err(|_| crypto("the key isn't base64url"))?;
    let sk = <Kem as hpke::Kem>::PrivateKey::from_bytes(&sk_bytes).map_err(|_| crypto("not an X25519 key"))?;
    let enc = <Kem as hpke::Kem>::EncappedKey::from_bytes(enc).map_err(|_| crypto("not an encapsulated key"))?;
    let plain = hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(&OpModeR::Base, &sk, &enc, HPKE_INFO, ct, collapse.as_bytes())
        .map_err(|_| crypto("this note isn't for this key"))?;
    serde_json::from_slice(&plain).map_err(|_| crypto("not a note"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn note() -> Note {
        Note {
            title: "? Fix the build".into(),
            body: "Which branch?".into(),
            target: serde_json::json!({ "task": "s1" }),
            choices: Vec::new(),
            reply: None,
        }
    }

    #[test]
    fn a_sealed_note_opens_with_its_key_for_its_task_only() {
        let mut rng = rand::rng();
        let (me, other) = (Keys::generate(&mut rng), Keys::generate(&mut rng));
        let sealed = seal(&note(), &me.public, "s1", &mut rng).unwrap();
        assert_eq!(open(&sealed, &me.secret, "s1").unwrap(), note());
        assert!(open(&sealed, &other.secret, "s1").is_err(), "another phone's key");
        assert!(open(&sealed, &me.secret, "s2").is_err(), "moved onto another task");
        let again = seal(&note(), &me.public, "s1", &mut rng).unwrap();
        assert_ne!(sealed, again, "a fresh ephemeral key each time");
        assert!(!String::from_utf8_lossy(&B64U.decode(&sealed).unwrap()).contains("Which branch"));
        assert!(seal(&note(), "nope!", "s1", &mut rng).is_err());
    }
}
