//! Where wisp's own copy of opencode lives (`accounts.install`): npm's, or the platform binary it downloads when
//! there's no npm (`Harness::binary_package`).

/// Where `accounts.install` puts it, in wisp's own tools folder: npm's copy, then the binary it unpacks itself when
/// there's no npm ([`binary_dest`]). wisp's home is `WISP_HOME`, else `.wisp` in the user's home (`HOME`, else
/// `USERPROFILE`: Windows rarely sets `HOME`, and reading only `HOME` missed the copy wisp had just installed there).
pub(crate) fn tools_bins(wisp_home: Option<String>, home: &str, platform: &str) -> Vec<String> {
    let base = wisp_home.filter(|h| !h.is_empty()).unwrap_or_else(|| format!("{home}/.wisp"));
    vec![format!("{base}/tools/node_modules/.bin/opencode"), format!("{base}/tools/{}", binary_dest(platform))]
}

/// The binary's place under the tools folder (`Harness::binary_package`'s `dest`).
pub(crate) fn binary_dest(platform: &str) -> String {
    format!("opencode/bin/opencode{}", if platform == "win32" { ".exe" } else { "" })
}

/// opencode's per-platform package on npm (`opencode-ai`'s optional dependencies): `opencode-<os>-<arch>`, with
/// `windows` for win32; its tarball holds the one executable.
pub(crate) fn platform_package(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "darwin" | "linux" => os,
        "win32" => "windows",
        _ => return None,
    };
    matches!(arch, "x64" | "arm64").then(|| format!("opencode-{os}-{arch}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows rarely sets HOME: the tools folder comes from the user's home as wisp resolves it (HOME, else
    /// USERPROFILE), so detect finds the copy accounts.install put there (was `/.wisp/tools/...`: "installed but
    /// doesn't start").
    #[test]
    fn detect_looks_in_wisps_tools_folder_under_the_users_home_and_wisp_home() {
        assert_eq!(
            tools_bins(None, "C:/Users/ana", "win32"),
            ["C:/Users/ana/.wisp/tools/node_modules/.bin/opencode", "C:/Users/ana/.wisp/tools/opencode/bin/opencode.exe"]
        );
        assert_eq!(tools_bins(Some(String::new()), "/home/a", "linux")[1], "/home/a/.wisp/tools/opencode/bin/opencode");
        assert_eq!(tools_bins(Some("/w".into()), "/home/a", "darwin")[0], "/w/tools/node_modules/.bin/opencode");
    }
}
