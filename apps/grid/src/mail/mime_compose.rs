//! A reply as the grid sends it: plain headers (an encoded word where they aren't ASCII), the thread's ids folded,
//! and the text and its HTML as one multipart body.
use base64::Engine as _;

use super::LINE_MAX;

fn ascii_header(s: &str) -> String {
    if s.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        s.to_owned()
    } else {
        format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(s))
    }
}

/// `Name: <id> <id>`, wrapped so no line gets long.
fn fold_ids(name: &str, list: &[String]) -> String {
    let head = format!("{name}:");
    let mut lines = vec![head.clone()];
    for id in list {
        let last = lines.last_mut().map_or(0, |l| l.len());
        if last + 1 + id.len() > LINE_MAX && lines.last().is_some_and(|l| *l != head) {
            lines.push(format!(" {id}"));
        } else if let Some(l) = lines.last_mut() {
            l.push(' ');
            l.push_str(id);
        }
    }
    lines.join("\r\n")
}

/// A reply to write.
#[derive(Debug, Clone, Default)]
pub struct Out<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    pub message_id: &'a str,
    pub in_reply_to: &'a str,
    pub references: &'a [String],
    /// RFC 5322's date
    pub date: &'a str,
}

/// A plain-text message, every line decided here. A stray line break in a header would write a header of the
/// sender's choosing, so what can't hold one is refused, and a subject is flattened.
pub fn compose_mail(o: &Out<'_>) -> Option<String> {
    let headers = [o.from, o.to, o.message_id, o.in_reply_to, o.date];
    if headers.iter().chain(o.references.iter().map(String::as_str).collect::<Vec<_>>().iter()).any(|h| h.contains(['\r', '\n'])) {
        return None;
    }
    let flat: String = o.subject.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_owned();
    let body = base64::engine::general_purpose::STANDARD.encode(o.text);
    let wrapped: Vec<&str> = body.as_bytes().chunks(76).filter_map(|c| std::str::from_utf8(c).ok()).collect();
    Some(
        [
            format!("From: {}", o.from),
            format!("To: {}", o.to),
            format!("Subject: {}", ascii_header(&flat)),
            format!("Date: {}", o.date),
            format!("Message-ID: {}", o.message_id),
            format!("In-Reply-To: {}", o.in_reply_to),
            fold_ids("References", o.references),
            "MIME-Version: 1.0".into(),
            "Content-Type: text/plain; charset=utf-8".into(),
            "Content-Transfer-Encoding: base64".into(),
            "Auto-Submitted: auto-replied".into(),
            "X-Auto-Response-Suppress: All".into(),
            String::new(),
            wrapped.join("\r\n"),
        ]
        .join("\r\n"),
    )
}
