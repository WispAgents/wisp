//! What a Windows process can't start without, for a child whose environment is otherwise cleared (an untrusted
//! exec plugin, ADR-0048; a grid process box): `SystemRoot` (without it Winsock doesn't load, so nothing resolves
//! `localhost` or listens), the system folders, `ComSpec`/`PATHEXT` (a `.cmd` runtime), the temp folder and the
//! processor count, and the child's own folder as its user profile (`HOME`'s counterpart, so nothing of the user's
//! profile is read). Only system variables, never the user's keys or the whole environment. Nothing off Windows:
//! `PATH` and `HOME` are enough there. Pure: the parent's environment comes in as `get`, the platform as `windows`.

/// The system variables copied from the parent, when it has them.
pub const WINDOWS_BASE_VARS: [&str; 8] =
    ["SystemRoot", "SystemDrive", "windir", "ComSpec", "PATHEXT", "TEMP", "TMP", "NUMBER_OF_PROCESSORS"];

/// The variables a cleared child gets on Windows: [`WINDOWS_BASE_VARS`] the parent has (none made up), plus
/// `USERPROFILE` = `profile` when given. Empty when `windows` is false.
pub fn windows_base_env(windows: bool, profile: Option<&str>, get: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    if !windows {
        return Vec::new();
    }
    let mut out: Vec<(String, String)> = WINDOWS_BASE_VARS.into_iter().filter_map(|k| get(k).map(|v| (k.to_owned(), v))).collect();
    if let Some(p) = profile {
        out.push(("USERPROFILE".to_owned(), p.to_owned()));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// A fake parent (a runner's values plus a user's secrets): the system variables come along, the profile is the
    /// child's own folder, a variable the parent doesn't have isn't made up, and no secret or user variable leaks.
    #[test]
    fn a_cleared_child_on_windows_gets_only_the_system_variables() {
        let parent: HashMap<&str, &str> = [
            ("SystemRoot", r"C:\Windows"),
            ("windir", r"C:\Windows"),
            ("ComSpec", r"C:\Windows\system32\cmd.exe"),
            ("PATHEXT", ".COM;.EXE;.BAT;.CMD"),
            ("TEMP", r"C:\Users\RUNNER~1\AppData\Local\Temp"),
            ("TMP", r"C:\Users\RUNNER~1\AppData\Local\Temp"),
            ("USERPROFILE", r"C:\Users\runneradmin"),
            ("APPDATA", r"C:\Users\runneradmin\AppData\Roaming"),
            ("ANTHROPIC_API_KEY", "sk-ant-secret"),
            ("GITHUB_TOKEN", "ghp_secret"),
            ("PATH", r"C:\Windows;C:\Users\runneradmin\bin"),
        ]
        .into_iter()
        .collect();
        let get = |k: &str| parent.get(k).map(|v| (*v).to_owned());
        let env: HashMap<String, String> = windows_base_env(true, Some(r"D:\plugins\acme"), get).into_iter().collect();
        assert_eq!(env.get("SystemRoot").map(String::as_str), Some(r"C:\Windows"), "Winsock loads");
        assert_eq!(env.get("ComSpec").map(String::as_str), Some(r"C:\Windows\system32\cmd.exe"));
        assert_eq!(env.get("TEMP").map(String::as_str), Some(r"C:\Users\RUNNER~1\AppData\Local\Temp"));
        assert_eq!(env.get("USERPROFILE").map(String::as_str), Some(r"D:\plugins\acme"), "its own, not the user's");
        assert!(!env.contains_key("SystemDrive"), "only what the parent has");
        for k in ["ANTHROPIC_API_KEY", "GITHUB_TOKEN", "APPDATA", "PATH"] {
            assert!(!env.contains_key(k), "{k} isn't a system variable");
        }
        assert!(env.keys().all(|k| k == "USERPROFILE" || WINDOWS_BASE_VARS.contains(&k.as_str())), "{env:?}");
        assert!(!windows_base_env(true, None, get).iter().any(|(k, _)| k == "USERPROFILE"), "no profile, none set");
        assert_eq!(windows_base_env(false, Some("/plugins/acme"), get), Vec::new(), "nothing off Windows");
    }
}
