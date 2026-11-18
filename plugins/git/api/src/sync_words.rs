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
    let files: Vec<&str> =
        git.lines().filter(|l| l.starts_with('\t') || l.starts_with("    ")).map(str::trim).filter(|l| !l.is_empty()).collect();
    if git.contains("would be overwritten") && !files.is_empty() {
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
    if lines.is_empty() { git.trim().to_owned() } else { lines.join("; ") }
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

    #[test]
    fn many_files_are_counted_not_listed() {
        let git = "error: Your local changes to the following files would be overwritten by merge:\n\ta\n\tb\n\tc\n\td\n\te\nAborting";
        assert!(not_updated("main", git).contains("a, b, c and 2 more"));
    }

    #[test]
    fn any_other_error_is_one_line_without_gits_prefixes() {
        let s = not_updated("main", "fatal: Not possible to fast-forward, aborting.\nhint: Diverging branches\n");
        assert_eq!(s, "Local main was not updated: Not possible to fast-forward, aborting.");
        assert!(!s.contains('\n'));
    }
}
