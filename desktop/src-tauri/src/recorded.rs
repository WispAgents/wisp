// The server a shell from npm runs when it starts on its own. `wisp` downloads this shell into
// <home>/desktop/<version>/ and starts it with WISP_BIN; the Dock, Spotlight and Finder start it with nothing, so
// `wisp` also records its server in <home>/desktop/server.json (crates/wisp/src/desktop.rs, record_server).
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub struct Recorded {
    pub bin: PathBuf,
    /// WISP_WEB_DIR: the npm package's web build, which the binary can't find from where npm put it.
    pub web: Option<PathBuf>,
    /// WISP_HOME: the one `wisp` used, so a Dock launch opens the same wisp a terminal does.
    pub home: Option<PathBuf>,
}

impl Recorded {
    pub fn env(&self) -> Vec<(&'static str, PathBuf)> {
        let web = self.web.clone().map(|w| ("WISP_WEB_DIR", w));
        let home = self.home.clone().map(|h| ("WISP_HOME", h));
        web.into_iter().chain(home).collect()
    }
}

pub fn parse(body: &[u8]) -> Option<Recorded> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    let path = |k: &str| v.get(k)?.as_str().filter(|s| !s.is_empty()).map(PathBuf::from);
    Some(Recorded { bin: path("bin")?, web: path("web"), home: path("home") })
}

/// Where to look: the `desktop` folder this shell sits in (whatever home `wisp` used, and a shell kept in the Dock
/// from an older version finds today's record), then the default home's.
pub fn files(exe: &Path, user_home: Option<&Path>) -> Vec<PathBuf> {
    let beside = exe.ancestors().find(|a| a.file_name().is_some_and(|n| n == "desktop")).map(|d| d.join("server.json"));
    let default = user_home.map(|h| h.join(".wisp").join("desktop").join("server.json"));
    let mut out: Vec<PathBuf> = beside.into_iter().chain(default).collect();
    out.dedup();
    out
}

/// The first record whose server is still there (an uninstalled npm package leaves a record naming nothing).
pub fn find(exe: &Path, user_home: Option<&Path>) -> Option<Recorded> {
    files(exe, user_home).iter().filter_map(|f| parse(&std::fs::read(f).ok()?)).find(|r| r.bin.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_in_any_versions_folder_finds_the_record_then_the_default_homes() {
        let exe = Path::new("/u/.wisp-x/desktop/1.0.0-alpha.9/wisp.app/Contents/MacOS/wisp");
        assert_eq!(
            files(exe, Some(Path::new("/u"))),
            [PathBuf::from("/u/.wisp-x/desktop/server.json"), PathBuf::from("/u/.wisp/desktop/server.json")]
        );
        let default = Path::new("/u/.wisp/desktop/1.0.0/wisp");
        assert_eq!(files(default, Some(Path::new("/u"))), [PathBuf::from("/u/.wisp/desktop/server.json")], "once");
        assert_eq!(files(Path::new("/Applications/wisp.app/Contents/MacOS/wisp"), None), Vec::<PathBuf>::new());
    }

    #[test]
    fn the_record_gives_the_server_its_web_build_and_home() {
        let r = parse(br#"{"bin":"/n/wisp","web":"/n/web","home":"/u/.wisp"}"#).unwrap();
        assert_eq!(r.bin, PathBuf::from("/n/wisp"));
        assert_eq!(r.env(), [("WISP_WEB_DIR", PathBuf::from("/n/web")), ("WISP_HOME", PathBuf::from("/u/.wisp"))]);
        assert_eq!(parse(br#"{"bin":"/n/wisp"}"#).unwrap().env(), Vec::new());
        for bad in [&b"{}"[..], br#"{"bin":""}"#, br#"{"bin":3}"#, b"not json", b""] {
            assert_eq!(parse(bad), None, "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn a_record_naming_a_removed_server_is_passed_over() {
        let dir = std::env::temp_dir().join(format!("wisp-recorded-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let desktop = dir.join("home/desktop");
        std::fs::create_dir_all(desktop.join("1.0.0")).unwrap();
        let exe = desktop.join("1.0.0/wisp");
        let bin = dir.join("wisp-bin");
        let write = |at: &Path, bin: &Path| std::fs::write(at, format!(r#"{{"bin":{}}}"#, serde_json::json!(bin))).unwrap();
        write(&desktop.join("server.json"), &dir.join("gone"));
        assert_eq!(find(&exe, None), None, "nothing to run");
        std::fs::write(&bin, "").unwrap();
        write(&desktop.join("server.json"), &bin);
        assert_eq!(find(&exe, None).map(|r| r.bin), Some(bin));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
