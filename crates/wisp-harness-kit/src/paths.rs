//! Project-relative paths and the size cap on file contents wisp keeps (v1 `server/paths.js`). Pure:
//! the driver resolves the cwd's real path once, before the run, and parsers get both spellings
//! (`/tmp` and `/private/tmp` on macOS). POSIX semantics, as `node:path` has on macOS and Linux, and
//! Windows semantics when the run's folder is a Windows one (`C:\…`, `\\server\share`, `\\?\C:\…`):
//! then `\` and `/` both separate, `\\?\` is dropped, the drive letter and the names compare without case,
//! and every path comes out with `/` (`C:/w/a.js`), which Windows opens as well. Decided by the strings,
//! never by `cfg`, so both shapes are tested on every OS.

/// Larger files are tracked as changed, but their contents aren't kept.
pub const MAX_SNAPSHOT: usize = 256 * 1024;

/// The run's folder as given and as the OS resolves it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunPaths {
    pub cwd: String,
    pub real: String,
}

impl RunPaths {
    pub fn new(cwd: impl Into<String>, real: impl Into<String>) -> Self {
        Self { cwd: cwd.into(), real: real.into() }
    }
    /// One spelling for both (tests, or a folder with no symlink on its way).
    pub fn same(cwd: impl Into<String>) -> Self {
        let cwd = cwd.into();
        Self { real: cwd.clone(), cwd }
    }
}

/// A Windows absolute path: `C:\…`, `C:/…`, `C:`, `\\server\share\…` or `\\?\…`.
pub fn is_windows(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with(r"\\")
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b.get(2).is_none_or(|c| matches!(c, b'/' | b'\\')))
}

/// Absolute on either OS.
pub fn is_absolute(p: &str) -> bool {
    p.starts_with('/') || is_windows(p)
}

/// A Windows path in one spelling: `\\?\` (and `\\?\UNC\`) dropped, `\` as `/`, the drive letter upper-case.
pub fn slashed(p: &str) -> String {
    let p = match p.strip_prefix(r"\\?\UNC\") {
        Some(r) => format!("//{r}"),
        None => p.strip_prefix(r"\\?\").unwrap_or(p).to_owned(),
    };
    let mut s = p.replace('\\', "/");
    if s.as_bytes().get(1) == Some(&b':') {
        s[..1].make_ascii_uppercase();
    }
    s
}

/// A normalised absolute path cut into its root (`""` on POSIX, `C:` or `//server/share` on Windows)
/// and its segments, `.` and `..` resolved.
fn split(p: &str, win: bool) -> (String, Vec<String>) {
    let s = if win { slashed(p) } else { p.to_owned() };
    let (root, rest) = if !win {
        (String::new(), s.as_str())
    } else if let Some(unc) = s.strip_prefix("//") {
        let mut it = unc.splitn(3, '/');
        let (server, share) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        (format!("//{server}/{share}"), it.next().unwrap_or(""))
    } else if s.as_bytes().get(1) == Some(&b':') {
        (s[..2].to_owned(), &s[2..])
    } else {
        (String::new(), s.as_str())
    };
    let mut parts: Vec<String> = Vec::new();
    for seg in rest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s.to_owned()),
        }
    }
    (root, parts)
}

/// `path.normalize` for an absolute path: `.` and `..` resolved, slashes collapsed, no trailing slash.
fn normalize_abs(p: &str, win: bool) -> String {
    let (root, parts) = split(p, win);
    let mut out = root;
    for s in &parts {
        out.push('/');
        out.push_str(s);
    }
    if parts.is_empty() {
        out.push('/');
    }
    out
}

/// `path.resolve(base, file)` (base is taken as absolute: a run's cwd always is). Windows when either is.
pub fn resolve(base: &str, file: &str) -> String {
    // a POSIX folder never turns a file Windows: there `\\x` is a name
    let win = is_windows(base) || (!base.starts_with('/') && is_windows(file));
    if (win && is_windows(file)) || (!win && file.starts_with('/')) {
        normalize_abs(file, win)
    } else if win && file.starts_with(['/', '\\']) {
        // `\x` on Windows: the base's drive (or share), then x
        let (root, _) = split(base, true);
        normalize_abs(&format!("{root}/{file}"), true)
    } else if is_absolute(base) {
        normalize_abs(&format!("{base}/{file}"), win)
    } else {
        normalize_abs(&format!("/{base}/{file}"), win)
    }
}

/// `path.relative(from, to)` for two absolute paths; `to` normalised when they share no root (two drives).
pub fn relative(from: &str, to: &str) -> String {
    let win = is_windows(from) || (!from.starts_with('/') && is_windows(to));
    let same = |a: &str, b: &str| if win { a.eq_ignore_ascii_case(b) } else { a == b };
    let ((fr, f), (tr, t)) = (split(from, win), split(to, win));
    if !same(&fr, &tr) {
        return normalize_abs(to, win);
    }
    let common = f.iter().zip(&t).take_while(|(a, b)| same(a, b)).count();
    let mut out: Vec<&str> = std::iter::repeat_n("..", f.len() - common).collect();
    out.extend(t[common..].iter().map(String::as_str));
    out.join("/")
}

/// Path relative to cwd (always with `/`), or the path as given if it lies outside (on Windows, in its one
/// spelling). Harnesses may report paths through the resolved cwd, so both spellings count. An empty path
/// stays empty.
pub fn rel(file: &str, paths: &RunPaths) -> String {
    if file.is_empty() {
        return String::new();
    }
    let abs = resolve(&paths.cwd, file);
    let bases: &[&str] = if paths.cwd == paths.real { &[&paths.cwd] } else { &[&paths.cwd, &paths.real] };
    for base in bases {
        let r = relative(base, &abs);
        if !r.is_empty() && !r.starts_with("..") && !is_absolute(&r) {
            return r;
        }
    }
    if is_windows(&paths.cwd) { slashed(file) } else { file.to_owned() }
}

/// `rel` over a JSON value (v1 `rel(input.file_path || …)`): falsy is `''`, the rest `String(v)`.
pub fn rel_value(file: &serde_json::Value, paths: &RunPaths) -> String {
    if !crate::js::truthy(file) {
        return String::new();
    }
    rel(&crate::js::string(file), paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_inside_and_outside() {
        let p = RunPaths::new("/tmp/w", "/private/tmp/w");
        assert_eq!(rel("/tmp/w/a/b.js", &p), "a/b.js");
        assert_eq!(rel("/private/tmp/w/a.js", &p), "a.js");
        assert_eq!(rel("a/./c/../b.js", &p), "a/b.js");
        assert_eq!(rel("/etc/hosts", &p), "/etc/hosts");
        assert_eq!(rel("../x", &p), "../x");
        assert_eq!(rel("/tmp/w", &p), "/tmp/w"); // the folder itself: '' is not a path
        assert_eq!(rel("", &p), "");
    }

    #[test]
    fn relative_walks_up() {
        assert_eq!(relative("/a/b", "/a/c/d"), "../c/d");
        assert_eq!(relative("/a", "/a"), "");
        assert_eq!(resolve("/a/b", "../c//d/"), "/a/c/d");
    }

    const WIN: &str = r"C:\Users\RUNNER~1\AppData\Local\Temp\wisp-serve-grok-1\proj";

    #[test]
    fn a_windows_folder_gives_relative_paths_with_forward_slashes() {
        let p = RunPaths::same(WIN);
        // what Windows CI saw: the recorded `@CWD@/math.js` joined onto a `\` folder
        assert_eq!(rel(&format!("{WIN}/math.js"), &p), "math.js");
        assert_eq!(rel(&format!(r"{WIN}\src\a.js"), &p), "src/a.js");
        assert_eq!(rel("notes.txt", &p), "notes.txt");
        assert_eq!(rel(r"src\b.js", &p), "src/b.js");
        assert_eq!(rel(r"src\.\c\..\b.js", &p), "src/b.js");
        // drive letter and names without case, `\\?\` dropped
        assert_eq!(rel(r"c:\users\runner~1\appdata\local\temp\wisp-serve-grok-1\proj\a.js", &p), "a.js");
        assert_eq!(rel(&format!(r"\\?\{WIN}\a.js"), &p), "a.js");
        assert_eq!(rel(r"\\?\C:\w\a.js", &RunPaths::same(r"C:\w")), "a.js");
        assert_eq!(rel(r"C:\w\a.js", &RunPaths::same(r"\\?\C:\w")), "a.js");
    }

    #[test]
    fn a_windows_path_outside_the_folder_keeps_one_spelling() {
        let p = RunPaths::same(r"C:\w");
        assert_eq!(rel(r"D:\other\x.js", &p), "D:/other/x.js");
        assert_eq!(rel(r"c:\x.js", &p), "C:/x.js");
        assert_eq!(rel(r"..\x", &p), "../x");
        assert_eq!(rel(r"C:\w", &p), "C:/w"); // the folder itself
        assert_eq!(rel(r"C:\wider\a.js", &p), "C:/wider/a.js"); // a prefix of a name is not inside
    }

    #[test]
    fn windows_resolve_and_relative() {
        assert_eq!(resolve(r"C:\w\proj", "notes.txt"), "C:/w/proj/notes.txt");
        assert_eq!(resolve(r"C:\w\proj", r"..\x\y.js"), "C:/w/x/y.js");
        assert_eq!(resolve(r"C:\w\proj", r"\top.js"), "C:/top.js");
        assert_eq!(resolve(r"C:\w", r"D:\a\..\b"), "D:/b");
        assert_eq!(resolve(r"\\srv\share\w", "a.js"), "//srv/share/w/a.js");
        assert_eq!(resolve(r"\\?\UNC\srv\share\w", "a.js"), "//srv/share/w/a.js");
        assert_eq!(rel(r"\\srv\share\w\s\a.js", &RunPaths::same(r"\\srv\share\w")), "s/a.js");
        assert_eq!(relative(r"C:\a\b", r"C:\a\c\d"), "../c/d");
        assert_eq!(relative(r"C:\a", r"D:\a"), "D:/a");
        assert_eq!(resolve(r"C:\w\", "."), resolve(r"C:\w", ""));
    }

    #[test]
    fn a_posix_folder_keeps_backslashes_in_names() {
        let p = RunPaths::same("/tmp/w");
        assert_eq!(rel(r"/tmp/w/a\b.js", &p), r"a\b.js");
        assert_eq!(rel(r"a\b.js", &p), r"a\b.js");
        assert_eq!(resolve("/tmp/w", r"\\srv\a"), r"/tmp/w/\\srv\a", "a name, not a share");
        assert!(is_windows(r"C:\x") && is_windows("c:/x") && is_windows(r"\\srv\s") && is_windows("C:"));
        assert!(!is_windows("/tmp/C:") && !is_windows("C:x") && !is_windows("ab:/x") && !is_windows("a.js"));
    }
}
