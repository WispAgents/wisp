//! A repo the user pasted (v1 parseRepo): `https://github.com/o/r(/tree/…)`, `github.com/o/r`,
//! `git@host:o/r.git`, `ssh://` `git://` `file://` URLs, and (shorthand) `o/r` for GitHub. None when
//! it isn't a repo, e.g. a path. Hand-parsed: the forms are few and a URL crate isn't worth a dependency.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoUrl {
    pub url: String,
    /// lowercased, without `www.`; "" for file URLs
    pub host: String,
    pub name: String,
    /// `owner/repo` on GitHub
    pub gh: Option<String>,
    pub path: String,
}

fn strip(p: &str) -> &str {
    let p = p.trim_end_matches('/');
    if p.len() >= 4 && p[p.len() - 4..].eq_ignore_ascii_case(".git") { &p[..p.len() - 4] } else { p }
}

fn of(url: String, host: &str, repo_path: &str) -> Option<RepoUrl> {
    let parts: Vec<&str> = strip(repo_path).split('/').filter(|s| !s.is_empty()).collect();
    let last = *parts.last()?;
    let gh = (host == "github.com" && parts.len() >= 2).then(|| format!("{}/{}", parts[0], parts[1]));
    let name = if gh.is_some() { parts[1] } else { last }.to_owned();
    Some(RepoUrl { url, host: host.to_owned(), name, gh, path: parts.join("/") })
}

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `scheme://[user@]host[:port]/path` → (scheme lowercased, host, path).
fn split_url(s: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = s.split_once("://")?;
    let (auth, path) = rest.find('/').map_or((rest, ""), |i| (&rest[..i], &rest[i..]));
    let host_port = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
    let host = if host_port.starts_with('[') {
        host_port.split_once(']').map_or(host_port, |(h, _)| h).trim_start_matches('[')
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    let path = path.split(['?', '#']).next().unwrap_or("");
    Some((scheme.to_ascii_lowercase(), host.to_ascii_lowercase(), path.to_owned()))
}

fn is_url(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    ["http://", "https://", "ssh://", "git://", "file://"].iter().any(|p| low.starts_with(p))
        || low.starts_with("github.com/")
        || low.starts_with("www.github.com/")
}

/// `C:…` (a drive letter and its colon, then a separator or nothing): a Windows path, whatever OS reads it.
fn is_drive(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b.get(2).is_none_or(|c| matches!(c, b'\\' | b'/'))
}

/// A folder as the `file://` URL git takes (2026-11-14): `/srv/x.git` → `file:///srv/x.git`; on Windows
/// `C:\a\b.git` (or `C:/a/b.git`) → `file:///C:/a/b.git` and `\\host\share\x` → `file://host/share/x`. Never
/// `file://C:\…`: git (and `parse_repo`) read `C:` there as a host.
#[must_use]
pub fn file_url(path: &str) -> String {
    let p = path.trim();
    if let Some(unc) = p.strip_prefix(r"\\") {
        return format!("file://{}", unc.replace('\\', "/"));
    }
    if is_drive(p) {
        return format!("file:///{}", p.replace('\\', "/"));
    }
    format!("file://{p}")
}

/// A `file://` URL spelled with a Windows path (`file://C:\x`, `file:///C:\x`, `file://C:/x`), as `file:///C:/x`.
fn windows_file_url(s: &str) -> Option<RepoUrl> {
    let rest = s.get(..7).filter(|h| h.eq_ignore_ascii_case("file://")).and(s.get(7..))?;
    let p = rest.strip_prefix('/').filter(|r| is_drive(r)).unwrap_or(rest);
    if !is_drive(p) {
        return None;
    }
    let p = p.replace('\\', "/");
    of(format!("file:///{p}"), "", &p)
}

pub fn parse_repo(input: &str, shorthand: bool) -> Option<RepoUrl> {
    let s = input.trim();
    if let Some(r) = windows_file_url(s) {
        return Some(r);
    }
    if is_url(s) {
        let full = if s.contains("://") { s.to_owned() } else { format!("https://{s}") };
        let (scheme, host, path) = split_url(&full)?;
        let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        if scheme == "file" {
            return of(full.clone(), "", &path);
        }
        if host.is_empty() {
            return None;
        }
        if host == "github.com" {
            let mut seg = path.split('/').filter(|x| !x.is_empty());
            let (o, r) = (seg.next()?, seg.next()?);
            return of(format!("https://github.com/{o}/{}.git", strip(r)), &host, &format!("{o}/{r}"));
        }
        let segments = path.split('/').filter(|x| !x.is_empty()).count();
        return if segments >= 2 || scheme != "https" { of(s.to_owned(), &host, &path) } else { None };
    }
    if let Some(r) = scp(s) {
        return Some(r);
    }
    if shorthand {
        let (o, r) = s.split_once('/')?;
        let ok_o = !o.is_empty() && o.chars().all(|c| word(c) || c == '-');
        let ok_r = !r.is_empty() && r.chars().all(|c| word(c) || c == '-' || c == '.');
        if ok_o && ok_r {
            return of(format!("https://github.com/{o}/{}.git", strip(r)), "github.com", &format!("{o}/{r}"));
        }
    }
    None
}

/// `user@host:path` (scp form), not `user@host://…`.
fn scp(s: &str) -> Option<RepoUrl> {
    let (user, rest) = s.split_once('@')?;
    let (host, path) = rest.split_once(':')?;
    let ok = |x: &str| !x.is_empty() && x.chars().all(|c| word(c) || c == '.' || c == '-');
    if !ok(user) || !ok(host) || path.is_empty() || path.starts_with("//") {
        return None;
    }
    of(s.to_owned(), &host.to_ascii_lowercase(), path)
}

/// `parse_repo`, plus a plain absolute path to a repo on disk (an origin can be one): `/srv/x`, `C:\x`, `\\host\share\x`.
pub fn repo_of(u: &str) -> Option<RepoUrl> {
    let u = u.trim();
    let path = u.starts_with('/') || u.starts_with(r"\\") || is_drive(u);
    parse_repo(u, false).or_else(|| path.then(|| parse_repo(&file_url(u), false)).flatten())
}

/// A repo's folder as git prints it (`rev-parse --show-toplevel` says `C:/a/b` on Windows), in the OS's own spelling:
/// a drive path gets `\`, any other is left as it is. Pure, so a Windows-shaped string reads the same everywhere.
#[must_use]
pub fn native_folder(path: &str) -> String {
    if is_drive(path) { path.replace('/', "\\") } else { path.to_owned() }
}

/// The same repo however it was spelled (https vs ssh, .git or not, the host's case; a Windows folder's case and
/// separators).
pub fn same_repo(a: &str, b: &str) -> bool {
    let key = |u: &str| {
        repo_of(u).map(|r| {
            r.gh.map_or_else(
                || if is_drive(&r.path) { format!("/{}", r.path.to_lowercase()) } else { format!("{}/{}", r.host, r.path) },
                |g| format!("github.com/{}", g.to_lowercase()),
            )
        })
    };
    matches!((key(a), key(b)), (Some(x), Some(y)) if x == y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gh(s: &str, short: bool) -> Option<String> {
        parse_repo(s, short).and_then(|r| r.gh)
    }

    #[test]
    fn github_forms() {
        for s in [
            "https://github.com/WispAgents/wisp",
            "https://github.com/WispAgents/wisp.git",
            "https://github.com/WispAgents/wisp/tree/main/server",
            "github.com/WispAgents/wisp/",
            "git@github.com:WispAgents/wisp.git",
        ] {
            assert_eq!(gh(s, false).as_deref(), Some("WispAgents/wisp"), "{s}");
        }
        assert_eq!(gh("WispAgents/wisp", true).as_deref(), Some("WispAgents/wisp"));
        assert_eq!(
            parse_repo("https://github.com/WispAgents/wisp/pull/3", false).map(|r| r.url).as_deref(),
            Some("https://github.com/WispAgents/wisp.git")
        );
    }

    #[test]
    fn other_hosts_and_ssh() {
        let r = parse_repo("https://gitlab.com/group/sub/app.git", false);
        assert_eq!(r.map(|r| (r.host, r.name, r.gh)), Some(("gitlab.com".into(), "app".into(), None)));
        assert_eq!(parse_repo("ssh://git@example.org:2222/team/tool.git", false).map(|r| r.name).as_deref(), Some("tool"));
        assert_eq!(parse_repo("file:///srv/git/proj.git", false).map(|r| (r.host, r.name)), Some((String::new(), "proj".into())));
    }

    #[test]
    fn paths_are_not_repos() {
        for p in ["~/code/app", "/Users/me/app", "./app", "app", "WispAgents/wisp", "https://github.com/M0xoo", "C:\\code\\app", ""] {
            assert_eq!(parse_repo(p, false), None, "{p}");
        }
    }

    #[test]
    fn same_repo_across_spellings() {
        assert!(same_repo("https://github.com/M0xoo/Wisp.git", "git@github.com:m0xoo/wisp"));
        assert!(same_repo("/srv/git/proj.git", "file:///srv/git/proj"));
        assert!(!same_repo("https://github.com/a/b", "https://github.com/a/c"));
        assert!(!same_repo("not a repo", "not a repo"));
    }

    /// `real_git.rs` on Windows: `file://C:\…` was refused as a repo URL (`C:` read as its host).
    #[test]
    fn a_windows_folder_is_a_file_url_git_takes_and_matches_its_other_spellings() {
        assert_eq!(file_url(r"C:\Users\me\origin.git"), "file:///C:/Users/me/origin.git");
        assert_eq!(file_url("C:/Users/me/origin.git"), "file:///C:/Users/me/origin.git");
        assert_eq!(file_url(r"\\nas\git\proj.git"), "file://nas/git/proj.git");
        assert_eq!(file_url("/srv/git/proj.git"), "file:///srv/git/proj.git");
        for s in [
            r"file://C:\Users\me\origin.git",
            "file://C:/Users/me/origin.git",
            r"file:///C:\Users\me\origin.git",
            "FILE:///C:/Users/me/origin.git",
        ] {
            let r = parse_repo(s, false).unwrap_or_else(|| panic!("{s}"));
            assert_eq!((r.url.as_str(), r.host.as_str(), r.name.as_str()), ("file:///C:/Users/me/origin.git", "", "origin"), "{s}");
        }
        assert!(same_repo(r"C:\Users\me\origin.git", r"file://C:\Users\me\origin.git"));
        assert!(same_repo("c:/users/ME/origin", "file:///C:/Users/me/origin.git"), "a drive's case and separators");
        assert!(!same_repo(r"C:\Users\me\origin.git", r"C:\Users\me\other.git"));
        assert!(same_repo(r"\\nas\git\proj.git", "file://nas/git/proj"));
        assert_eq!(parse_repo(r"C:\code\app", false), None, "a bare path stays a path");
        assert!(!is_drive("C:x") && is_drive("C:") && is_drive("c:/") && !is_drive("/C:"));
    }

    #[test]
    fn a_windows_top_folder_gets_its_own_separators() {
        assert_eq!(native_folder("C:/Users/me/repo"), r"C:\Users\me\repo");
        assert_eq!(native_folder("/home/me/repo"), "/home/me/repo");
        assert_eq!(native_folder(""), "");
    }
}

/// Where people keep their repos (v1 services/places.js `REPO_ROOTS`, without Documents, Downloads, Desktop).
pub const REPO_ROOTS: [&str; 11] = ["code", "Code", "dev", "Developer", "projects", "Projects", "src", "repos", "work", "git", "GitHub"];

/// Where a pasted repo URL is cloned (v1 `cloneRoot`): `git.cloneDir` (`~/` is home), else the first of the usual
/// repo folders that is there, else `~/code`. `is_dir` asks the file system (sans-IO here).
pub fn clone_root(setting: Option<&str>, home: &str, is_dir: impl Fn(&str) -> bool) -> String {
    let home = home.trim_end_matches('/');
    if let Some(d) = setting.map(str::trim).filter(|d| !d.is_empty()) {
        return match d.strip_prefix("~") {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{home}{rest}"),
            _ => d.to_owned(),
        };
    }
    REPO_ROOTS.iter().map(|r| format!("{home}/{r}")).find(|d| is_dir(d)).unwrap_or_else(|| format!("{home}/code"))
}

#[cfg(test)]
mod clone_root_tests {
    use super::*;

    #[test]
    fn a_repo_is_cloned_where_settings_say_else_the_first_usual_folder_else_code() {
        assert_eq!(clone_root(Some("~/src/oss"), "/Users/a", |_| false), "/Users/a/src/oss");
        assert_eq!(clone_root(Some("/data/repos"), "/Users/a", |_| false), "/data/repos");
        assert_eq!(clone_root(Some(" "), "/Users/a", |d| d == "/Users/a/Developer"), "/Users/a/Developer");
        assert_eq!(clone_root(None, "/Users/a/", |d| d == "/Users/a/dev" || d == "/Users/a/work"), "/Users/a/dev", "in v1's order");
        assert_eq!(clone_root(None, "/Users/a", |_| false), "/Users/a/code");
    }
}
