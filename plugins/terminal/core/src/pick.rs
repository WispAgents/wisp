//! Preview's element picker (v1 server/runs.js): the panel shows the task's app through a small proxy on a port of its
//! own (its own origin, as the app's: wisp's API stays out of the app's reach), the same app minus X-Frame-Options
//! and CSP frame-ancestors, with one script added to its HTML pages ([`PICK_JS`]) that lets the panel pick an element.
//! This is what the proxy decides; the host moves the bytes. Pure.

/// Where the proxy serves the picker's script in the app's own pages.
pub const PICK_PATH: &str = "/__wisp/pick.js";
/// v1's `PICK_JS`, verbatim.
pub const PICK_JS: &str = include_str!("../assets/pick.js");
/// A page bigger than this goes through as it is, without the picker (v1 `HTML_MAX`).
pub const HTML_MAX: u64 = 8 * 1024 * 1024;
/// v1 `PROXY_BASE`: a task's proxy is `42000 + hash(id) % 900` (its app is on 41000 + the same: never the same port).
const PROXY_BASE: u16 = 42_000;
const SPAN: u32 = 900;

/// v1 `pickPort`: how many ports after the task's own are tried before any free one the system gives.
pub const PROXY_TRIES: u32 = 60;

fn proxy_hash(id: &str) -> u32 {
    id.chars().fold(7_u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c)))
}

/// The port task `id`'s Preview proxy listens on when it's free (stable per task, as the app's: the same origin each
/// time, so the app keeps its storage across restarts). Only the first try: the port a proxy runs on is the one it bound.
#[must_use]
pub fn proxy_port_for(id: &str) -> u16 {
    PROXY_BASE + u16::try_from(proxy_hash(id) % SPAN).unwrap_or(0)
}

/// The ports task `id`'s proxy tries, in order (v1 `pickPort`): its own, then the next ones in the span (wrapping),
/// [`PROXY_TRIES`] in all, the same each time. A port another program holds, or one in a range the system reserves
/// (Windows' Hyper-V/WinNAT exclusions), is passed over; when none is free the host asks for any free one.
pub fn proxy_ports_for(id: &str) -> impl Iterator<Item = u16> {
    let h = proxy_hash(id);
    (0..PROXY_TRIES).map(move |i| PROXY_BASE + u16::try_from(h.wrapping_add(i) % SPAN).unwrap_or(0))
}

/// Only the page's own frame: a request whose Host isn't this computer's loopback (a site elsewhere reaching the port
/// through a name that points here: DNS rebinding) is refused.
#[must_use]
pub fn loopback_host(host: &str) -> bool {
    let Some((name, port)) = host.rsplit_once(':') else { return false };
    !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit()) && matches!(name, "localhost" | "127.0.0.1" | "[::1]")
}

/// The app's headers, framable by the panel: X-Frame-Options gone, CSP's frame-ancestors gone (every other rule
/// stays), a CSP left empty dropped. Names are compared in any case.
#[must_use]
pub fn frameable(headers: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (k, v) in headers {
        let name = k.to_ascii_lowercase();
        if name == "x-frame-options" {
            continue;
        }
        if name == "content-security-policy" {
            let kept =
                v.split(';').filter(|d| !d.trim_start().to_ascii_lowercase().starts_with("frame-ancestors")).collect::<Vec<_>>().join(";");
            let kept = kept.trim();
            if !kept.is_empty() {
                out.push((k.clone(), kept.to_owned()));
            }
            continue;
        }
        out.push((k.clone(), v.clone()));
    }
    out
}

/// Whether an answer gets the picker: a GET's HTML page, not too big to hold (no length said counts as small, as v1).
#[must_use]
pub fn takes_picker(method: &str, content_type: Option<&str>, length: Option<u64>) -> bool {
    method.eq_ignore_ascii_case("GET")
        && content_type.is_some_and(|t| t.trim_start().to_ascii_lowercase().starts_with("text/html"))
        && length.unwrap_or(0) <= HTML_MAX
}

/// The picker's script tag, as early in the page as it can go: right after `<head …>`, else `<html …>`, else first.
/// A page that has it already is left alone.
#[must_use]
pub fn with_picker(html: &str) -> String {
    let tag = format!("<script src=\"{PICK_PATH}\"></script>");
    if html.contains(PICK_PATH) {
        return html.to_owned();
    }
    let lower = html.to_ascii_lowercase();
    let after = |name: &str| -> Option<usize> {
        let mut from = 0;
        while let Some(i) = lower[from..].find(&format!("<{name}")).map(|i| i + from) {
            let next = lower.as_bytes().get(i + 1 + name.len()).copied();
            if matches!(next, Some(b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/')) {
                return lower[i..].find('>').map(|j| i + j + 1);
            }
            from = i + 1;
        }
        None
    };
    match after("head").or_else(|| after("html")) {
        Some(at) => format!("{}{tag}{}", &html[..at], &html[at..]),
        None => format!("{tag}{html}"),
    }
}

/// The text without terminal colours and cursor moves (v1 cleanLine's ANSI): `ESC [ … letter`, `ESC ] … BEL|ESC \\`,
/// and the two-byte `ESC x` forms.
#[must_use]
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('[') => {
                for x in it.by_ref() {
                    if ('@'..='~').contains(&x) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(x) = it.next() {
                    if x == '\u{7}' || (x == '\u{1b}' && it.peek() == Some(&'\\')) {
                        if x == '\u{1b}' {
                            it.next();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The first local address the app's output says it listens on (v1 urlIn, over its cleaned lines): its port.
#[must_use]
pub fn local_port_in(text: &str) -> Option<u16> {
    first_local(&strip_ansi(text)).map(|(p, _)| p)
}

/// How much of the output already seen [`PortScan`] keeps to scan again with the next chunk: enough for an address
/// line (colours and all) cut anywhere between two chunks.
const SCAN_TAIL: usize = 256;

/// [`local_port_in`] over output that comes in chunks, each looked at once: the app's address is found as it's
/// printed and kept, so it isn't looked for again in the kept screen (a megabyte, under the terminals' lock, on every
/// Preview request), and it's still known once that line has scrolled out of the screen.
#[derive(Debug, Default, Clone)]
pub struct PortScan {
    /// the end of what came so far, raw: an address or a colour split between two chunks is whole again
    tail: String,
    found: Option<u16>,
    /// a port whose digits ran to the end of what came: the next chunk may add digits (`51` + `73`)
    maybe: Option<u16>,
}

impl PortScan {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks at the next chunk of output; the app's port once it's known for sure (and every time after).
    pub fn push(&mut self, chunk: &str) -> Option<u16> {
        if self.found.is_some() {
            return self.found;
        }
        let text = format!("{}{chunk}", self.tail);
        match first_local(&strip_ansi(&text)) {
            Some((p, false)) => {
                self.found = Some(p);
                self.maybe = None;
                self.tail = String::new();
                return self.found;
            }
            Some((p, true)) => self.maybe = Some(p),
            None => self.maybe = None,
        }
        let mut cut = text.len().saturating_sub(SCAN_TAIL);
        while !text.is_char_boundary(cut) {
            cut += 1;
        }
        text[cut..].clone_into(&mut self.tail);
        None
    }

    /// The port the output says so far: the sure one, else one that may still grow digits (an app whose last line
    /// is its address and nothing after it).
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.found.or(self.maybe)
    }
}

/// The first local address in cleaned text: its port, and whether its digits run to the end of the text.
fn first_local(text: &str) -> Option<(u16, bool)> {
    for scheme in ["http://", "https://"] {
        let mut from = 0;
        while let Some(i) = text[from..].find(scheme).map(|i| i + from) {
            let rest = &text[i + scheme.len()..];
            // `[::1]:5173`: the host's own colons are inside its brackets
            let host_end =
                if rest.starts_with('[') { rest.find(']').map(|b| b + 1).filter(|&b| rest[b..].starts_with(':')) } else { rest.find(':') };
            if let Some(h) = host_end {
                let host = &rest[..h];
                let digits: String = rest[h + 1..].chars().take_while(char::is_ascii_digit).collect();
                if matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0" | "[::1]" | "[::]")
                    && (2..=5).contains(&digits.len())
                    && let Ok(p) = digits.parse::<u16>()
                    && p > 0
                {
                    return Some((p, h + 1 + digits.len() == rest.len()));
                }
            }
            from = i + scheme.len();
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn h(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    /// v1 runs.js frameable/withPicker/proxy: the app framable by the panel (X-Frame-Options and frame-ancestors out,
    /// every other rule kept), the script once and as early as it can go, only HTML GETs under 8 MB, only a loopback
    /// Host, the proxy's port beside the app's and never the same.
    #[test]
    fn the_preview_proxy_frames_the_app_and_adds_the_picker_as_v1() {
        let out = frameable(&h(&[
            ("X-Frame-Options", "DENY"),
            ("Content-Security-Policy", "default-src 'self'; frame-ancestors 'none'; img-src *"),
            ("content-type", "text/html"),
        ]));
        assert_eq!(out, h(&[("Content-Security-Policy", "default-src 'self'; img-src *"), ("content-type", "text/html")]));
        assert_eq!(
            frameable(&h(&[("content-security-policy", "frame-ancestors 'self'")])),
            Vec::<(String, String)>::new(),
            "nothing left: no CSP"
        );

        let tag = "<script src=\"/__wisp/pick.js\"></script>";
        assert_eq!(
            with_picker("<!doctype html><html lang=en><head><title>x</title></head></html>"),
            format!("<!doctype html><html lang=en><head>{tag}<title>x</title></head></html>")
        );
        assert_eq!(with_picker("<HTML><body>hi</body></HTML>"), format!("<HTML>{tag}<body>hi</body></HTML>"));
        assert_eq!(with_picker("<header>not the head</header><p>x</p>"), format!("{tag}<header>not the head</header><p>x</p>"));
        assert_eq!(with_picker(&format!("<head>{tag}</head>")), format!("<head>{tag}</head>"), "already there: once");

        assert!(takes_picker("GET", Some("text/html; charset=utf-8"), Some(1200)));
        assert!(takes_picker("get", Some("text/html"), None));
        assert!(!takes_picker("POST", Some("text/html"), None));
        assert!(!takes_picker("GET", Some("application/json"), None));
        assert!(!takes_picker("GET", Some("text/html"), Some(HTML_MAX + 1)));

        assert!(loopback_host("localhost:42123") && loopback_host("127.0.0.1:42123") && loopback_host("[::1]:42123"));
        assert!(
            !loopback_host("evil.example:42123")
                && !loopback_host("localhost")
                && !loopback_host("localhost.evil.example:1")
                && !loopback_host("localhost:abc")
        );

        let (app, proxy) = (crate::run::port_for("s1"), proxy_port_for("s1"));
        assert!((42_000..42_900).contains(&proxy) && app != proxy);
        assert_eq!(proxy_port_for("s1"), proxy, "the same task, the same port");
        let tries: Vec<u16> = proxy_ports_for("s1").collect();
        assert_eq!(tries.first(), Some(&proxy), "its own port first");
        assert_eq!(tries.len(), 60);
        assert!(tries.iter().all(|p| (42_000..42_900).contains(p)), "never on an app's port");
        assert_eq!(tries.iter().collect::<std::collections::HashSet<_>>().len(), 60, "each tried once");
        assert_eq!(tries, proxy_ports_for("s1").collect::<Vec<_>>(), "the same order each time");
        assert!(PICK_JS.contains("wisp: 'pick'") && PICK_JS.contains("window.parent"));
    }

    /// v1 urlIn: the first local address the app's output prints, its port; a remote address or no port isn't one.
    #[test]
    fn the_app_s_address_is_the_first_local_one_it_prints() {
        assert_eq!(
            local_port_in("  VITE v7 ready\n  ➜  Local:   http://localhost:5173/\n  ➜  Network: http://192.168.1.4:5173/"),
            Some(5173)
        );
        assert_eq!(local_port_in("listening on http://127.0.0.1:8080"), Some(8080));
        assert_eq!(local_port_in("see https://example.com:443/docs"), None);
        assert_eq!(local_port_in("http://localhost/ no port"), None);
        assert_eq!(local_port_in("http://0.0.0.0:3000 and http://localhost:4000"), Some(3000));
        assert_eq!(local_port_in("Listening on http://[::1]:8000/"), Some(8000));
        // vite colours the port: the colours go first
        assert_eq!(
            local_port_in("  \u{1b}[32m➜\u{1b}[39m  Local:   \u{1b}[36mhttp://localhost:\u{1b}[1m5173\u{1b}[22m/\u{1b}[39m"),
            Some(5173)
        );
        assert_eq!(strip_ansi("\u{1b}]0;title\u{7}ok \u{1b}[1mbold\u{1b}[0m"), "ok bold");
    }

    /// The address is found as output comes, chunk by chunk: one split between two chunks (in the middle of a colour
    /// too) is whole, a port whose digits may go on waits for them, and the port is kept after the line has left the
    /// kept screen (a megabyte later, where reading the screen again found nothing).
    #[test]
    fn the_app_s_address_is_found_across_chunks_and_kept() {
        let line = "  \u{1b}[32m➜\u{1b}[39m  Local:   \u{1b}[36mhttp://localhost:\u{1b}[1m5173\u{1b}[22m/\u{1b}[39m\r\n";
        for cut in 1..line.len() {
            if !line.is_char_boundary(cut) {
                continue;
            }
            let mut s = PortScan::new();
            let first = s.push(&line[..cut]);
            assert!(first.is_none() || first == Some(5173), "cut at {cut}: {first:?}");
            assert_eq!(s.push(&line[cut..]), Some(5173), "cut at {cut}");
        }

        let mut s = PortScan::new();
        assert_eq!(s.push("listening on http://127.0.0.1:51"), None, "more digits may come");
        assert_eq!(s.port(), Some(51), "what it says so far");
        assert_eq!(s.push("73\n"), Some(5173));

        let mut s = PortScan::new();
        assert_eq!(s.push("see https://example.com:443/docs\n"), None);
        assert_eq!(s.push("http://0.0.0.0:3000 and http://localhost:4000\n"), Some(3000), "the first local one");
        let noise = "x".repeat(64 * 1024) + "\n";
        for _ in 0..20 {
            assert_eq!(s.push(&noise), Some(3000));
        }
        assert_eq!(s.push("now http://localhost:9999/\n"), Some(3000), "the first one stays");

        // the one-shot reading, for comparison: the line has left a megabyte's screen
        let mut r = crate::replay::Replay::new(crate::replay::KEEP);
        r.push("http://localhost:5173/\n");
        for _ in 0..20 {
            r.push(&noise);
        }
        assert_eq!(local_port_in(r.read()), None);
    }
}
