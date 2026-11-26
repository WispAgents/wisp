//! The Steward's reads of gh, parsed: a PR's review threads (GraphQL), a check's latest run on the base, the query
//! string, and who reviewed.

use std::fmt::Write as _;

use super::OpenThread;

/// The GraphQL query for a PR's review threads (`gh api graphql -F o= -F r= -F n=`).
pub const THREADS_QUERY: &str = "query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){author{login} \
reviewThreads(first:100){nodes{id isResolved isOutdated path line comments(last:20){nodes{author{login __typename} body createdAt}}}}}}}";

/// The threads that count from the query's answer: unresolved, not outdated, with a person in them (a thread only
/// bots wrote is left out), in the order GitHub lists them.
pub fn threads_from(j: &serde_json::Value) -> Vec<OpenThread> {
    let pr = &j["data"]["repository"]["pullRequest"];
    let nodes = pr["reviewThreads"]["nodes"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = Vec::new();
    for t in nodes {
        if t["isResolved"].as_bool() == Some(true) || t["isOutdated"].as_bool() == Some(true) {
            continue;
        }
        let comments = t["comments"]["nodes"].as_array().map_or(&[][..], Vec::as_slice);
        let person = |c: &serde_json::Value| c["author"]["__typename"].as_str() != Some("Bot");
        if !comments.iter().any(person) {
            continue;
        }
        let said: Vec<(String, String)> = comments
            .iter()
            .filter(|c| person(c))
            .map(|c| {
                (c["author"]["login"].as_str().unwrap_or("someone").to_owned(), c["body"].as_str().unwrap_or_default().trim().to_owned())
            })
            .collect();
        out.push(OpenThread {
            id: t["id"].as_str().unwrap_or_default().to_owned(),
            path: t["path"].as_str().unwrap_or_default().to_owned(),
            line: t["line"].as_u64().and_then(|l| u32::try_from(l).ok()),
            comments: said,
            last_at: comments.last().and_then(|c| c["createdAt"].as_str()).unwrap_or_default().to_owned(),
        });
    }
    out
}

/// `repos/{o}/{r}/commits/{base}/check-runs?check_name=…`: the base's latest run of that check failed.
pub fn base_failed(j: &serde_json::Value) -> bool {
    j["check_runs"].as_array().and_then(|r| r.first()).is_some_and(|r| matches!(r["conclusion"].as_str(), Some("failure" | "timed_out")))
}

/// A query string value, percent-encoded.
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Who reviewed, as the green line names it: "Codex · gpt-6.2", or the harness alone.
pub fn reviewer_name(name: &str, model: &str) -> String {
    if model.is_empty() { name.to_owned() } else { format!("{name} · {model}") }
}
