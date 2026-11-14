//! Where a page in this box shows (ADR-0064): a cloud box's grid tells its wisp `WISP_PREVIEW_URL`
//! (`https://p{port}--ada.grid.example`), and `grid-client.status` / the `grid-client.cloud` row carry it as
//! `preview`, which the answer's `preview` block reads (grid-client's UI maps `localhost:<port>` onto it). Fail
//! closed: anything but a box's own, well-formed template is "" (the block then shows the plain URL).

/// The template a box's wisp publishes, or "": only in a box (`WISP_BOX=1`, not a sandbox), only
/// `http(s)://p{port}--<host>` with nothing after the host (no path, query, fragment or user).
pub fn template(env: &dyn Fn(&str) -> Option<String>) -> String {
    if env("WISP_BOX").as_deref() != Some("1") || env("WISP_SANDBOX").as_deref() == Some("1") {
        return String::new();
    }
    let raw = env("WISP_PREVIEW_URL").unwrap_or_default();
    let raw = raw.trim().trim_end_matches('/');
    let Some((scheme, rest)) = raw.split_once("://") else { return String::new() };
    let host = rest.strip_prefix("p{port}--").unwrap_or_default();
    let host_ok = !host.is_empty()
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'))
        && !host.starts_with(['.', '-', ':']);
    if matches!(scheme, "http" | "https") && host_ok { format!("{scheme}://p{{port}}--{host}") } else { String::new() }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let v: Vec<(String, String)> = pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        move |k| v.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone())
    }

    #[test]
    fn a_box_publishes_its_grid_s_template_and_nothing_else_does() {
        let boxed = |url: &str| template(&env(&[("WISP_BOX", "1"), ("WISP_PREVIEW_URL", url)]));
        assert_eq!(boxed("https://p{port}--ada.grid.example"), "https://p{port}--ada.grid.example");
        assert_eq!(boxed("http://p{port}--ada.wisp.localhost:8080/"), "http://p{port}--ada.wisp.localhost:8080");
        for bad in [
            "",
            "ftp://p{port}--ada.grid.example",
            "https://ada.grid.example",
            "https://p{port}--",
            "https://p{port}--ada.grid.example/path",
            "https://p{port}--ada.grid.example?x=1",
            "https://p{port}--evil@ada.grid.example",
            "https://p{port}--ada.{port}.example",
            "javascript:alert(1)",
        ] {
            assert_eq!(boxed(bad), "", "{bad}");
        }
        // not a box (a laptop with the variable set by hand), or a sandbox: nothing
        assert_eq!(template(&env(&[("WISP_PREVIEW_URL", "https://p{port}--ada.grid.example")])), "");
        let sb = env(&[("WISP_BOX", "1"), ("WISP_SANDBOX", "1"), ("WISP_PREVIEW_URL", "https://p{port}--ada.grid.example")]);
        assert_eq!(template(&sb), "");
    }
}
