//! Audit H HS12: right after a restart the first page's event stream from a cloud computer got a 401. The proxy made
//! the box's direct channel from the token cache, which is empty until a peer's secret is read, so the request went to
//! the grid without its token. Every place that makes a channel to send on reads the peer's secret first.
#![allow(clippy::unwrap_used, clippy::panic)]

/// The source of each file that makes channels, and every line that makes one.
fn channel_lines() -> Vec<(&'static str, usize, String)> {
    let files = [
        ("proxy.rs", include_str!("../src/proxy.rs")),
        ("service.rs", include_str!("../src/service.rs")),
        ("commands.rs", include_str!("../src/commands.rs")),
        ("carry.rs", include_str!("../src/carry.rs")),
    ];
    let mut out = Vec::new();
    for (name, src) in files {
        let lines: Vec<&str> = src.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            // `usable` only asks whether one could be made; nothing is sent on it
            if l.contains(".channel(") && !l.contains(".is_ok()") && !l.trim_start().starts_with("//") {
                let before = lines[i.saturating_sub(8)..i].join("\n");
                out.push((name, i + 1, before));
            }
        }
    }
    out
}

#[test]
fn every_channel_made_to_send_on_reads_the_peers_secret_first_audit_h_hs12() {
    let all = channel_lines();
    assert!(all.len() >= 5, "the channel makers were found: {}", all.len());
    for (file, line, before) in all {
        assert!(before.contains("secret_of("), "{file}:{line} makes a channel without reading the peer's secret first");
    }
}
