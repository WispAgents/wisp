use super::*;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-svc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[tokio::test]
async fn secrets_round_trip_in_v1_shape_and_0600() {
    let d = tmp("secrets");
    let s = FileSecrets::new(&d);
    s.set("token", "a").await.unwrap();
    s.set("telegram/bot", "b").await.unwrap();
    assert_eq!(s.get("token").await.unwrap().as_deref(), Some("a"));
    assert_eq!(s.get("wisp/token").await.unwrap().as_deref(), Some("a"));
    assert_eq!(s.keys("").await.unwrap(), vec!["telegram/bot", "token"]);
    let on_disk: serde_json::Value = serde_json::from_slice(&std::fs::read(d.join("secrets.json")).unwrap()).unwrap();
    assert_eq!(on_disk, serde_json::json!({"telegram": {"bot": "b"}, "wisp": {"token": "a"}}));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(d.join("secrets.json")).unwrap().permissions().mode() & 0o777, 0o600);
    }
    s.delete("telegram/bot").await.unwrap();
    assert_eq!(s.keys("tele").await.unwrap(), Vec::<String>::new());
    assert!(!d.join("secrets.json.tmp").exists(), "the tmp file is renamed over, never left behind");
    // a second handle (another process later) reads what the first wrote
    assert_eq!(FileSecrets::new(&d).get("token").await.unwrap().as_deref(), Some("a"));
}

#[tokio::test]
async fn a_v1_file_reads_as_it_is_and_odd_values_are_dropped() {
    let d = tmp("v1");
    std::fs::write(d.join("secrets.json"), r#"{"wisp":{"grid":"g","n":3},"slack":{"hook":"h"}}"#).unwrap();
    let s = FileSecrets::new(&d);
    assert_eq!(s.get("grid").await.unwrap().as_deref(), Some("g"));
    assert_eq!(s.get("n").await.unwrap(), None);
    assert_eq!(s.get("slack/hook").await.unwrap().as_deref(), Some("h"));
}

#[tokio::test]
async fn corrupt_file_is_never_overwritten() {
    let d = tmp("corrupt");
    for bytes in [&b"{not json"[..], b"[1,2]"] {
        std::fs::write(d.join("secrets.json"), bytes).unwrap();
        let s = FileSecrets::new(&d);
        assert!(s.get("k").await.is_err(), "a corrupt file isn't read as empty");
        assert!(s.keys("").await.is_err());
        assert!(s.set("a.b", "v").await.is_err(), "set refuses instead of writing over it");
        assert!(s.delete("a.b").await.is_err());
        assert_eq!(std::fs::read(d.join("secrets.json")).unwrap(), bytes, "the original bytes are untouched");
    }
}

#[tokio::test]
async fn one_bad_namespace_keeps_the_rest() {
    let d = tmp("badns");
    std::fs::write(d.join("secrets.json"), r#"{"x":"str","wisp":{"k":"v"}}"#).unwrap();
    let s = FileSecrets::new(&d);
    assert_eq!(s.get("k").await.unwrap().as_deref(), Some("v"));
    s.set("wisp/k2", "w").await.unwrap();
    let on_disk: serde_json::Value = serde_json::from_slice(&std::fs::read(d.join("secrets.json")).unwrap()).unwrap();
    assert_eq!(on_disk["wisp"], serde_json::json!({"k": "v", "k2": "w"}));
}

#[cfg(unix)]
#[tokio::test]
async fn read_error_is_not_cached() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("unreadable");
    let f = d.join("secrets.json");
    std::fs::write(&f, r#"{"wisp":{"k":"v"}}"#).unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o000)).unwrap();
    // root reads a 000 file anyway; there's nothing to test then
    if std::fs::read(&f).is_ok() {
        return;
    }
    let s = FileSecrets::new(&d);
    assert!(s.get("k").await.is_err());
    assert!(s.set("k3", "x").await.is_err(), "nothing is written over a file that couldn't be read");
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(s.get("k").await.unwrap().as_deref(), Some("v"), "the next call reads the file again");
}

#[tokio::test]
async fn a_missing_file_reads_empty_and_set_creates_it_0600() {
    let d = tmp("missing");
    let s = FileSecrets::new(&d);
    assert_eq!(s.get("k").await.unwrap(), None);
    assert_eq!(s.keys("").await.unwrap(), Vec::<String>::new());
    s.set("k", "v").await.unwrap();
    assert_eq!(FileSecrets::new(&d).get("k").await.unwrap().as_deref(), Some("v"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(d.join("secrets.json")).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[tokio::test]
async fn concurrent_writes_all_land() {
    let d = tmp("concurrent");
    let s = FileSecrets::new(&d);
    let mut tasks = Vec::new();
    for i in 0..20 {
        let s = s.clone();
        tasks.push(tokio::spawn(async move { s.set(&format!("k{i}"), "v").await }));
    }
    for t in tasks {
        t.await.unwrap().unwrap();
    }
    assert_eq!(FileSecrets::new(&d).keys("k").await.unwrap().len(), 20);
}

#[test]
fn home_is_wisp_home_or_dot_wisp() {
    assert_eq!(home(Some("/x"), Some(PathBuf::from("/u"))), PathBuf::from("/x"));
    assert_eq!(home(Some(""), Some(PathBuf::from("/u"))), PathBuf::from("/u/.wisp"));
    assert_eq!(home(None, Some(PathBuf::from("/u"))), PathBuf::from("/u/.wisp"));
}

#[test]
fn clock_moves() {
    assert!(SystemClock.now_ms() > 1_700_000_000_000);
}

/// v1 #567 (F-set-7): on Windows a save's rename over a file something holds for a moment failed with EPERM and
/// the change was refused. A held file (permission denied, busy) is waited out; it gives up after 10 tries, and
/// an error that isn't a held file fails at once.
#[tokio::test]
async fn a_save_over_a_briefly_held_file_waits_it_out_instead_of_failing() {
    use std::io::{Error, ErrorKind};
    use std::sync::atomic::{AtomicU32, Ordering};
    let tries = AtomicU32::new(0);
    let held_twice = rename_retrying(|| {
        let n = tries.fetch_add(1, Ordering::SeqCst);
        async move { if n < 2 { Err(Error::from(ErrorKind::PermissionDenied)) } else { Ok(()) } }
    });
    held_twice.await.unwrap();
    assert_eq!(tries.load(Ordering::SeqCst), 3, "two held tries, then the rename lands");

    tries.store(0, Ordering::SeqCst);
    let busy = rename_retrying(|| {
        tries.fetch_add(1, Ordering::SeqCst);
        async { Err(Error::from(ErrorKind::ResourceBusy)) }
    });
    assert_eq!(busy.await.unwrap_err().kind(), ErrorKind::ResourceBusy);
    assert_eq!(tries.load(Ordering::SeqCst), 10, "held for good: it gives up after 10 tries");

    tries.store(0, Ordering::SeqCst);
    let gone = rename_retrying(|| {
        tries.fetch_add(1, Ordering::SeqCst);
        async { Err(Error::from(ErrorKind::NotFound)) }
    });
    assert_eq!(gone.await.unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(tries.load(Ordering::SeqCst), 1, "not a held file: no retry");
}

/// ADR-0148: on a team hub's box the team's secrets (env vars the grid names in `WISP_HUB_SECRETS`) answer as
/// `${secret:NAME}` where the file has none; the file's own value wins; anywhere else, nothing from the env.
#[tokio::test]
async fn a_hubs_team_secrets_answer_as_secrets_and_the_file_wins() {
    static HUB: [(&str, &str); 5] =
        [("WISP_HUB", "1"), ("WISP_HUB_SECRETS", "HOOK_SECRET, GH_TOKEN,"), ("HOOK_SECRET", "s3"), ("GH_TOKEN", "ghp"), ("PATH", "/bin")];
    static NOT_A_HUB: [(&str, &str); 2] = [("WISP_HUB_SECRETS", "PATH"), ("PATH", "/bin")];
    let d = std::env::temp_dir().join(format!("wisp-team-secrets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let env =
        |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| (*v).to_owned());
    let team = super::team_secrets(&env(&HUB));
    assert_eq!(team.keys().collect::<Vec<_>>(), ["GH_TOKEN", "HOOK_SECRET"], "only the names the grid gave");
    assert!(super::team_secrets(&env(&NOT_A_HUB)).is_empty(), "not a hub: none");
    let s = FileSecrets::with_team(&d, team);
    assert_eq!(s.get("HOOK_SECRET").await.unwrap().as_deref(), Some("s3"));
    assert_eq!(s.get("PATH").await.unwrap(), None);
    s.set("HOOK_SECRET", "mine").await.unwrap();
    assert_eq!(s.get("HOOK_SECRET").await.unwrap().as_deref(), Some("mine"), "the file's own wins");
    assert_eq!(s.keys("").await.unwrap(), ["GH_TOKEN", "HOOK_SECRET"]);
    let _ = std::fs::remove_dir_all(&d);
}
