//! The grid's own images (`apps/grid/assets`: the logo, the screenshots of the app and a report, the agents'
//! logos, the share card), built into the binary so a grid is one file to deploy, served at `/_grid/a/<name>` on
//! every address it answers (v1 grid/server.js `asset`). `/favicon.svg` and `/favicon.ico` are the logo too.

/// `(name, extension) → bytes`, every file in `apps/grid/assets`.
const FILES: [(&str, &str, &[u8]); 11] = [
    ("claudecode-color", "svg", include_bytes!("../assets/claudecode-color.svg")),
    ("codex-color", "svg", include_bytes!("../assets/codex-color.svg")),
    ("cursor", "svg", include_bytes!("../assets/cursor.svg")),
    ("opencode", "svg", include_bytes!("../assets/opencode.svg")),
    ("icon", "svg", include_bytes!("../assets/icon.svg")),
    ("dark-app", "webp", include_bytes!("../assets/dark-app.webp")),
    ("light-app", "webp", include_bytes!("../assets/light-app.webp")),
    ("dark-report", "webp", include_bytes!("../assets/dark-report.webp")),
    ("light-report", "webp", include_bytes!("../assets/light-report.webp")),
    ("og", "png", include_bytes!("../assets/og.png")),
    // the favicon: browsers ask the root for it whatever the page says
    ("favicon", "svg", include_bytes!("../assets/icon.svg")),
];

/// The content type for an extension the grid serves.
pub fn content_type(ext: &str) -> &'static str {
    match ext {
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

/// An image by path: `/_grid/a/<name>.<ext>`, `/favicon.svg` or `/favicon.ico`. Its bytes and content type.
pub fn by_path(path: &str) -> Option<(&'static [u8], &'static str)> {
    let (name, ext) = match path {
        "/favicon.svg" | "/favicon.ico" => ("favicon", "svg"),
        _ => crate::hosts::asset_path(path)?,
    };
    FILES.iter().find(|(n, e, _)| *n == name && *e == ext).map(|(_, e, b)| (*b, content_type(e)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_image_a_page_names_is_served() {
        for p in [
            "/_grid/a/icon.svg",
            "/_grid/a/light-app.webp",
            "/_grid/a/dark-app.webp",
            "/_grid/a/light-report.webp",
            "/_grid/a/dark-report.webp",
            "/_grid/a/claudecode-color.svg",
            "/_grid/a/codex-color.svg",
            "/_grid/a/opencode.svg",
            "/_grid/a/cursor.svg",
            "/_grid/a/og.png",
            "/favicon.svg",
            "/favicon.ico",
        ] {
            let (b, ct) = by_path(p).unwrap_or_else(|| panic!("{p} isn't served"));
            assert!(!b.is_empty() && ct.starts_with("image/"), "{p}");
        }
        assert_eq!(by_path("/_grid/a/light-app.webp").unwrap().1, "image/webp");
        assert!(by_path("/_grid/a/../Cargo.toml").is_none());
        assert!(by_path("/_grid/a/nope.svg").is_none());
        assert!(by_path("/_grid/a/icon.exe").is_none());
    }
}
