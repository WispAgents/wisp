//! An OIDC `id_token`, checked (v1 grid/auth/jwt.js): RS256 or ES256 by a key from the issuer's JWKS, its issuer,
//! audience, nonce and expiry.
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use serde_json::Value;
use sha2::Sha256;

use crate::error::{Result, bad};

fn b64(part: &str) -> Result<Vec<u8>> {
    B64URL.decode(part.trim_end_matches('=')).map_err(|_| bad("The ID token isn't readable.", 401))
}

/// An OIDC ID token's claims, once its signature (RS256 or ES256, by `kid` in `jwks`), issuer, audience, nonce and
/// times check out (a minute of clock skew).
pub fn verify_id_token(token: &str, jwks: &Value, issuers: &[String], audience: &str, nonce: &str, now_ms: i64) -> Result<Value> {
    let refuse = |why: &str| bad(format!("The sign-in couldn't be checked: {why}."), 401);
    let parts: Vec<&str> = token.split('.').collect();
    let [h, p, sig] = parts.as_slice() else { return Err(refuse("not a JWT")) };
    let header: Value = serde_json::from_slice(&b64(h)?).map_err(|_| refuse("its header isn't JSON"))?;
    let claims: Value = serde_json::from_slice(&b64(p)?).map_err(|_| refuse("its claims aren't JSON"))?;
    let alg = header.get("alg").and_then(Value::as_str).unwrap_or_default();
    let kid = header.get("kid").and_then(Value::as_str);
    let keys = jwks.get("keys").and_then(Value::as_array).cloned().unwrap_or_default();
    let key = keys
        .iter()
        .find(|k| kid.is_none_or(|kid| k.get("kid").and_then(Value::as_str) == Some(kid)))
        .ok_or_else(|| refuse("its key isn't the issuer's"))?;
    let signed = format!("{h}.{p}");
    let sig = b64(sig)?;
    let ok = match alg {
        "RS256" => rs256(key, signed.as_bytes(), &sig),
        "ES256" => es256(key, signed.as_bytes(), &sig),
        _ => return Err(refuse(&format!("it's signed with {alg}, not RS256 or ES256"))),
    };
    if !ok {
        return Err(refuse("its signature is wrong"));
    }
    let iss = claims.get("iss").and_then(Value::as_str).unwrap_or_default();
    if !issuers.iter().any(|i| i == iss) {
        return Err(refuse("another issuer signed it"));
    }
    let aud_ok = match claims.get("aud") {
        Some(Value::String(a)) => a == audience,
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str() == Some(audience)),
        _ => false,
    };
    if !aud_ok {
        return Err(refuse("it's for another app"));
    }
    if claims.get("nonce").and_then(Value::as_str) != Some(nonce) {
        return Err(refuse("its nonce isn't this sign-in's"));
    }
    let now = now_ms / 1_000;
    let skew = 60;
    if claims.get("exp").and_then(Value::as_i64).is_none_or(|e| e + skew < now) {
        return Err(refuse("it expired"));
    }
    if claims.get("iat").and_then(Value::as_i64).is_some_and(|i| i - skew > now) {
        return Err(refuse("it's from the future"));
    }
    Ok(claims)
}

fn rs256(jwk: &Value, msg: &[u8], sig: &[u8]) -> bool {
    use rsa::signature::Verifier as _;
    let n = jwk.get("n").and_then(Value::as_str).and_then(|s| B64URL.decode(s).ok());
    let e = jwk.get("e").and_then(Value::as_str).and_then(|s| B64URL.decode(s).ok());
    let (Some(n), Some(e)) = (n, e) else { return false };
    let Ok(key) = rsa::RsaPublicKey::new(rsa::BigUint::from_bytes_be(&n), rsa::BigUint::from_bytes_be(&e)) else { return false };
    let Ok(sig) = rsa::pkcs1v15::Signature::try_from(sig) else { return false };
    rsa::pkcs1v15::VerifyingKey::<Sha256>::new(key).verify(msg, &sig).is_ok()
}

fn es256(jwk: &Value, msg: &[u8], sig: &[u8]) -> bool {
    use p256::ecdsa::signature::Verifier as _;
    let coord = |k: &str| jwk.get(k).and_then(Value::as_str).and_then(|s| B64URL.decode(s).ok()).filter(|b| b.len() == 32);
    let (Some(x), Some(y)) = (coord("x"), coord("y")) else { return false };
    let point = p256::EncodedPoint::from_affine_coordinates(x.as_slice().into(), y.as_slice().into(), false);
    let Ok(key) = p256::ecdsa::VerifyingKey::from_encoded_point(&point) else { return false };
    let Ok(sig) = p256::ecdsa::Signature::from_slice(sig) else { return false };
    key.verify(msg, &sig).is_ok()
}
