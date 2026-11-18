//! `edge`'s tests.
use super::*;

#[test]
fn queries_decode() {
    assert_eq!(query("a=1&next=%2Factivate%3Fcode%3DX", "next").as_deref(), Some("/activate?code=X"));
    assert_eq!(split("/api/shares/x?ref=v2"), (vec!["api", "shares", "x"], "ref=v2"));
}
