//! What the merge toast says when the user's base couldn't be brought up to date (audit A #16): git's error is several
//! lines ("error: Your local changes to the following files would be overwritten by merge:", a tab-indented file per
//! line, "Please commit your changes or stash them…"), and the toast shows one: it stopped at the colon. One line, in
//! words, the files named.

/// `Local <base> was not updated: <why>`, on one line.
pub fn not_updated(base: &str, git: &str) -> String {
    format!("Local {base} was not updated: {}", reason(git))
}

/// Git's error on one line: local changes in the way named with their files, else its lines joined.
fn reason(git: &str) -> String {
    let mut files: Vec<&str> =
        git.lines().filter(|l| l.starts_with('\t') || l.starts_with("    ")).map(str::trim).filter(|l| !l.is_empty()).collect();
    // git's error on one line (the git plugin's `why`: "…would be overwritten by merge: a, b"), the files after its colon
    if files.is_empty()
        && let Some((_, list)) = git.split_once("by merge:").or_else(|| git.split_once("by checkout:"))
    {
        files = list.split(',').map(str::trim).filter(|f| !f.is_empty()).collect();
    }
    if git.contains("would be overwritten") && files.is_empty() {
        return "your uncommitted changes are in the way. Commit or stash them, then pull.".to_owned();
    }
    if git.contains("would be overwritten") {
        let shown: Vec<&str> = files.iter().take(3).copied().collect();
        let more = files.len() - shown.len();
        let list = if more > 0 { format!("{} and {more} more", shown.join(", ")) } else { shown.join(", ") };
        return format!("your uncommitted changes to {list} are in the way. Commit or stash them, then pull.");
    }
    let lines: Vec<String> = git
        .lines()
        .map(|l| l.trim().trim_start_matches("error:").trim_start_matches("fatal:").trim().trim_end_matches(':').to_owned())
        .filter(|l| !l.is_empty() && !l.starts_with("Aborting") && !l.starts_with("hint:"))
        .collect();
    let said = if lines.is_empty() { git.trim().to_owned() } else { lines.join("; ") };
    // a sentence that ends (audit E9: git's line had no full stop after "Local main was not updated:")
    if said.ends_with(['.', '!', '?']) || said.is_empty() { said } else { format!("{said}.") }
}

#[cfg(test)]
mod tests {
    use super::not_updated;

    #[test]
    fn local_changes_in_the_way_are_named_on_one_line() {
        let git = "error: Your local changes to the following files would be overwritten by merge:\n\tdocs/dev.md\n\tmath.js\nPlease commit your changes or stash them before you merge.\nAborting\n";
        let s = not_updated("main", git);
        assert_eq!(
            s,
            "Local main was not updated: your uncommitted changes to docs/dev.md, math.js are in the way. Commit or stash them, then pull."
        );
        assert!(!s.contains('\n'));
    }

    /// Audit E9: what the merge gets is git's error on one line, the files after its colon (`porcelain::why`).
    #[test]
    fn the_one_line_form_names_its_files_too_audit_e9() {
        let s = not_updated("main", "error: Your local changes to the following files would be overwritten by merge: docs/dev.md, math.js");
        assert_eq!(
            s,
            "Local main was not updated: your uncommitted changes to docs/dev.md, math.js are in the way. Commit or stash them, then pull."
        );
        // cut at the colon, with no files: no dangling colon, a sentence that ends
        let s = not_updated("main", "error: Your local changes to the following files would be overwritten by merge:");
        assert_eq!(s, "Local main was not updated: your uncommitted changes are in the way. Commit or stash them, then pull.");
    }

    #[test]
    fn many_files_are_counted_not_listed() {
        let git = "error: Your local changes to the following files would be overwritten by merge:\n\ta\n\tb\n\tc\n\td\n\te\nAborting";
        assert!(not_updated("main", git).contains("a, b, c and 2 more"));
    }

    #[test]
    fn any_other_error_is_one_line_without_gits_prefixes() {
        let s = not_updated("main", "fatal: Not possible to fast-forward, aborting.\nhint: Diverging branches\n");
        assert_eq!(s, "Local main was not updated: Not possible to fast-forward, aborting.");
        assert_eq!(
            not_updated("main", "fatal: couldn't find remote ref main"),
            "Local main was not updated: couldn't find remote ref main."
        );
        assert!(!s.contains('\n'));
    }
}
