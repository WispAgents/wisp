//! What this wisp calls itself to its clients (the welcome's `name`): the name a person knows the computer by.
//! `$HOSTNAME` alone named a grid box after its container id ("1959efa12b19", read on the phone as a computer) and a
//! Mac's app-started server "this computer" (the shell sets `$HOSTNAME`, a launched app never sees it).
//! Order: `WISP_NAME`, a grid box's "Cloud", the Mac's Computer Name (Settings → General → About), the host name.
use std::time::Duration;

/// The name for `box_mode` (a grid box), else this computer's, else "this computer".
pub async fn of_this_wisp(box_mode: bool) -> String {
    if let Some(n) = std::env::var("WISP_NAME").ok().as_deref().and_then(label) {
        return n;
    }
    if box_mode {
        return "Cloud".into();
    }
    if cfg!(target_os = "macos")
        && let Some(n) = run("scutil", &["--get", "ComputerName"]).await.as_deref().and_then(label)
    {
        return n;
    }
    if let Some(n) = run("hostname", &[]).await.as_deref().and_then(label) {
        return n;
    }
    ["HOSTNAME", "COMPUTERNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().as_deref().and_then(label))
        .unwrap_or_else(|| "this computer".into())
}

async fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = tokio::process::Command::new(cmd);
    c.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(2), c.output()).await.ok()?.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A name worth showing: trimmed, without mDNS's `.local`; none for an empty one, `localhost`, or a container id
/// (12 or 64 hex digits, what Docker sets as a container's host name).
pub fn label(raw: &str) -> Option<String> {
    let n = raw.trim();
    let n = n.strip_suffix(".local").unwrap_or(n).trim();
    let container = matches!(n.len(), 12 | 64) && n.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (!n.is_empty() && !n.eq_ignore_ascii_case("localhost") && !container).then(|| n.to_owned())
}

#[cfg(test)]
mod tests {
    use super::label;

    /// The phone listed a grid box as "1959efa12b19": its container id, which Docker gives as the host name.
    #[test]
    fn a_container_id_is_no_name() {
        assert_eq!(label("1959efa12b19\n"), None);
        assert_eq!(label(&"ab".repeat(32)), None);
        assert_eq!(label("localhost"), None);
        assert_eq!(label("  "), None);
    }

    #[test]
    fn a_host_name_reads_as_people_know_it() {
        assert_eq!(label("Mokhless-MacBook-Pro.local\n").as_deref(), Some("Mokhless-MacBook-Pro"));
        assert_eq!(label("Ada’s MacBook Pro\n").as_deref(), Some("Ada’s MacBook Pro"));
        assert_eq!(label("devbox").as_deref(), Some("devbox"));
        assert_eq!(label("cafe12345678").as_deref(), None, "12 hex digits read as a container id");
        assert_eq!(label("studio-2024").as_deref(), Some("studio-2024"));
    }
}
