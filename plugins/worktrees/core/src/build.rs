//! Build output in wisp's worktrees (ADR-0144): the folders a build makes and the next build makes again (cargo's
//! `target/`, `node_modules/`, Next's and Turbo's caches). They can take most of a folder (one left-over worktree held a
//! 120 GB `target/` beside 580 changed screenshots), and removing them loses no work, so a low disk takes them first and
//! leaves the folder, its branch and its changes where they are. Pure: the host reads `du` and asks git whether each is
//! ignored right before it goes.

use std::collections::HashMap;

use wisp_worktrees_api::Disk;

/// The folder names that are build output wherever they sit in a worktree (`target` only with cargo's `CACHEDIR.TAG`,
/// checked by the host; each only when git ignores it there).
pub const NAMES: [&str; 4] = ["target", "node_modules", ".next", ".turbo"];

/// How deep `du` looks, counted from wisp's worktrees folder: `<tree>/spikes/<x>/target` is 4.
pub const DEPTH: u32 = 4;

/// A trim never takes a folder whose task ran or whose files moved this recently: a build may be under way in it.
pub const TRIM_IDLE: i64 = 3_600_000;

/// Each worktree's build folders and their bytes, by the worktree's path.
pub type Builds = HashMap<String, Vec<(String, u64)>>;

/// `du -k -d DEPTH <dir>`: each worktree's size (the lines one level under `dir`) and its build output (the lines
/// named in [`NAMES`], none inside another: `node_modules/x/node_modules` is in its parent's count).
pub fn parse_du(out: &str, dir: &str) -> (HashMap<String, u64>, Builds) {
    let dir = dir.trim_end_matches('/');
    let mut sizes = HashMap::new();
    let mut found: Vec<(String, String, u64)> = Vec::new();
    for l in out.lines() {
        let Some((k, p)) = l.split_once('\t') else { continue };
        let Ok(kb) = k.trim().parse::<u64>() else { continue };
        let p = p.trim_end_matches('/');
        let Some(rel) = p.strip_prefix(dir).and_then(|r| r.strip_prefix('/')) else { continue };
        let parts: Vec<&str> = rel.split('/').collect();
        let Some(tree) = parts.first().filter(|t| !t.is_empty()) else { continue };
        let tree = format!("{dir}/{tree}");
        if parts.len() == 1 {
            sizes.insert(tree, kb * 1024);
        } else if parts.last().is_some_and(|n| NAMES.contains(n)) {
            found.push((tree, p.to_owned(), kb * 1024));
        }
    }
    // shortest first, so a nested one finds its parent already taken
    found.sort_by_key(|(_, p, _)| p.len());
    let mut builds: Builds = HashMap::new();
    for (tree, path, bytes) in found {
        let list = builds.entry(tree).or_default();
        if !list.iter().any(|(q, _)| path.starts_with(&format!("{q}/"))) {
            list.push((path, bytes));
        }
    }
    (sizes, builds)
}

/// A folder whose build output a low disk may take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trim {
    /// the worktree's path
    pub tree: String,
    pub bytes: u64,
    /// when its task last ran or its files last moved
    pub used_at: i64,
    /// no task owns it
    pub leftover: bool,
}

/// The worktrees whose build output goes, when the disk is under `low`: left-over folders first, then idle tasks', the
/// oldest first in each, until `enough` is free. Nothing when the disk is fine or unknown.
pub fn plan(cands: &[Trim], disk: Option<Disk>, low: u64, enough: u64, now: i64) -> Vec<String> {
    let Some(d) = disk.filter(|d| d.free < low) else { return Vec::new() };
    let mut order: Vec<&Trim> = cands.iter().filter(|c| c.bytes > 0 && now - c.used_at >= TRIM_IDLE).collect();
    order.sort_by_key(|c| (!c.leftover, c.used_at));
    let mut free = d.free;
    let mut out = Vec::new();
    for c in order {
        if free >= enough {
            break;
        }
        out.push(c.tree.clone());
        free += c.bytes;
    }
    out
}

/// Whether `rel` (a build folder's path inside its worktree) may go: a plain relative path, no `..`, named in [`NAMES`].
pub fn removable(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.starts_with('/')
        && rel.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
        && rel.rsplit('/').next().is_some_and(|n| NAMES.contains(&n))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;
    const KB_GB: u64 = 1024 * 1024;

    #[test]
    fn du_gives_each_worktree_and_its_build_output_with_nested_ones_in_their_parent() {
        let out = format!(
            "{}\t/w/a/target/debug\n{}\t/w/a/target\n{}\t/w/a/node_modules/x/node_modules\n{}\t/w/a/node_modules\n\
             {}\t/w/a/spikes/s/target\n{}\t/w/a/src\n{}\t/w/a\n{}\t/w/b\n{}\t/w\n",
            100 * KB_GB,
            120 * KB_GB,
            1024,
            2048,
            KB_GB,
            10,
            125 * KB_GB,
            5,
            125 * KB_GB
        );
        let (sizes, builds) = parse_du(&out, "/w/");
        assert_eq!(sizes.get("/w/a"), Some(&(125 * GB)));
        assert_eq!(sizes.get("/w/b"), Some(&(5 * 1024)));
        assert!(!sizes.contains_key("/w"));
        let mut a = builds["/w/a"].clone();
        a.sort();
        assert_eq!(
            a,
            vec![
                ("/w/a/node_modules".to_owned(), 2048 * 1024),
                ("/w/a/spikes/s/target".to_owned(), GB),
                ("/w/a/target".to_owned(), 120 * GB),
            ]
        );
        assert!(!builds.contains_key("/w/b"));
    }

    #[test]
    fn du_lines_outside_the_folder_or_unreadable_are_skipped() {
        let (sizes, builds) = parse_du("x\t/w/a\n12\t/elsewhere/target\n7\t/w/a\n", "/w");
        assert_eq!(sizes.len(), 1);
        assert!(builds.is_empty());
    }

    fn t(tree: &str, gb: u64, used_at: i64, leftover: bool) -> Trim {
        Trim { tree: tree.into(), bytes: gb * GB, used_at, leftover }
    }

    const NOW: i64 = 1_000 * TRIM_IDLE;

    #[test]
    fn a_low_disk_takes_leftovers_first_then_the_oldest_idle_until_enough() {
        let c = [
            t("task-old", 4, 0, false),
            t("left-new", 3, NOW - 2 * TRIM_IDLE, true),
            t("left-old", 3, 0, true),
            t("task-new", 9, NOW - 2 * TRIM_IDLE, false),
        ];
        let disk = Some(Disk { free: 5 * GB, total: 500 * GB });
        assert_eq!(plan(&c, disk, 10 * GB, 15 * GB, NOW), vec!["left-old", "left-new", "task-old"]);
    }

    #[test]
    fn nothing_goes_when_the_disk_is_fine_unknown_or_the_folder_was_just_used() {
        let c = [t("a", 50, 0, true), t("busy", 50, NOW - 1, false)];
        assert!(plan(&c, Some(Disk { free: 20 * GB, total: 500 * GB }), 10 * GB, 15 * GB, NOW).is_empty());
        assert!(plan(&c, None, 10 * GB, 15 * GB, NOW).is_empty());
        assert_eq!(plan(&c, Some(Disk { free: GB, total: 500 * GB }), 10 * GB, 15 * GB, NOW), vec!["a"]);
    }

    #[test]
    fn only_a_named_folder_inside_the_worktree_may_go() {
        assert!(removable("target"));
        assert!(removable("spikes/s/target"));
        assert!(removable("node_modules"));
        assert!(!removable("src"));
        assert!(!removable("../target"));
        assert!(!removable("/target"));
        assert!(!removable("a//target"));
        assert!(!removable(""));
    }
}
