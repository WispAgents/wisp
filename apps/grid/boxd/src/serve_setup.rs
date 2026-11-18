//! boxd setting up: its folders root's alone, the users' network, the socket only the grid's group reaches, and
//! moving homes left by an older layout.
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use super::{Boxd, Opts, is_root};
use crate::run::is_id;

/// boxd's folder and every folder above it: root's and writable by root only, or someone could swap a box's home
/// for a link to anywhere.
pub fn root_only_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // canonicalize: boxd is Linux-only and has no dependencies on purpose (ADR-0014): no verbatim prefix to strip
    let mut d = std::fs::canonicalize(dir).map_err(|e| e.to_string())?;
    loop {
        let m = std::fs::symlink_metadata(&d).map_err(|e| e.to_string())?;
        if m.file_type().is_symlink() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err(format!("{} must be root's and writable by root only: boxd keeps the users' homes under it.", d.display()));
        }
        if !d.pop() {
            break;
        }
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())
}

fn group_id(name: &str) -> Result<u32, String> {
    let text = std::fs::read_to_string("/etc/group").map_err(|e| e.to_string())?;
    text.lines()
        .map(|l| l.split(':').collect::<Vec<_>>())
        .find(|f| f.first() == Some(&name))
        .and_then(|f| f.get(2).and_then(|g| g.parse().ok()))
        .ok_or_else(|| format!("There's no group \"{name}\" (--group: the grid's user's group)."))
}

/// The users' network: containers can't talk to each other.
fn ensure_network(o: &Opts) -> Result<(), String> {
    let run = |args: &[&str]| {
        Command::new(&o.docker)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "The container driver needs Docker.".to_owned())
    };
    if run(&["network", "inspect", &o.container.network])?.success() {
        return Ok(());
    }
    let ok = run(&[
        "network",
        "create",
        "--driver",
        "bridge",
        "-o",
        "com.docker.network.bridge.enable_icc=false",
        "-o",
        "com.docker.network.bridge.name=br-wisp",
        &o.container.network,
    ])?
    .success();
    if ok { Ok(()) } else { Err(format!("docker network create {} failed", o.container.network)) }
}

/// Listens on the socket (0660, the grid's group) until the process is stopped.
pub fn serve(o: Opts) -> Result<(Arc<Boxd>, UnixListener), String> {
    let root = is_root();
    if root {
        root_only_dir(&o.boxes)?;
    } else {
        std::fs::create_dir_all(&o.boxes).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&o.boxes, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let gid = if root && !o.group.is_empty() { Some(group_id(&o.group)?) } else { None };
    if let Err(e) = ensure_network(&o) {
        eprintln!("boxd: {e}");
    }
    if std::fs::symlink_metadata(&o.socket).is_ok_and(|m| {
        use std::os::unix::fs::FileTypeExt as _;
        m.file_type().is_socket()
    }) {
        let _ = std::fs::remove_file(&o.socket);
    }
    if let Some(dir) = o.socket.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        // nobody but root and the group reaches the socket while its own mode is being set
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o750)).map_err(|e| e.to_string())?;
        if let Some(g) = gid {
            std::os::unix::fs::chown(dir, Some(0), Some(g)).map_err(|e| e.to_string())?;
        }
    }
    let l = UnixListener::bind(&o.socket).map_err(|e| format!("{}: {e}", o.socket.display()))?;
    if let Some(g) = gid {
        std::os::unix::fs::chown(&o.socket, Some(0), Some(g)).map_err(|e| e.to_string())?;
    }
    std::fs::set_permissions(&o.socket, std::fs::Permissions::from_mode(0o660)).map_err(|e| e.to_string())?;
    Ok((Boxd::new(o), l))
}

/// Accepts connections, each on its own thread.
pub fn accept(b: &Arc<Boxd>, l: &UnixListener) {
    for c in l.incoming().flatten() {
        let b = b.clone();
        std::thread::spawn(move || b.handle(c));
    }
}

/// Homes kept at the old place (`<data>/users/<id>/home`, in the grid's own folder) move into boxd's, with the grid
/// stopped. → (moved, skipped).
pub fn migrate(o: &Opts) -> Result<(Vec<String>, Vec<String>), String> {
    let ps = Command::new(&o.docker).args(["ps", "-q", "--filter", "name=wisp-u-"]).output();
    if ps.as_ref().is_ok_and(|p| p.status.success() && !String::from_utf8_lossy(&p.stdout).trim().is_empty()) {
        return Err("Boxes are running: stop the grid first (systemctl stop wisp-grid).".into());
    }
    if is_root() {
        root_only_dir(&o.boxes)?;
    } else {
        std::fs::create_dir_all(&o.boxes).map_err(|e| e.to_string())?;
    }
    let users = o.data.join("users");
    let mut ids: Vec<String> =
        std::fs::read_dir(&users).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    ids.sort();
    let (mut moved, mut skipped) = (Vec::new(), Vec::new());
    let real_dir = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
    for id in ids {
        let from = users.join(&id).join("home");
        if !is_id(&id) || !from.exists() {
            continue;
        }
        let to = o.boxes.join(&id).join("home");
        let why = if !real_dir(&users.join(&id)) || !real_dir(&from) {
            format!("{} isn't a folder", from.display())
        } else if to.exists() {
            format!("{} is already there", to.display())
        } else {
            String::new()
        };
        if !why.is_empty() {
            eprintln!("  ! {id}: skipped, {why}");
            skipped.push(id);
            continue;
        }
        let parent = o.boxes.join(&id);
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(&from, &to) {
            // another disk: mv copies it across (owners, modes and links kept) and removes the old one
            let ok = e.raw_os_error() == Some(18) && Command::new("mv").arg(&from).arg(&to).status().is_ok_and(|s| s.success());
            if !ok {
                let _ = std::fs::remove_dir_all(&parent);
                return Err(format!("{} → {}: {e}", from.display(), to.display()));
            }
        }
        eprintln!("  ✓ {id}: {} → {}", from.display(), to.display());
        moved.push(id);
    }
    Ok((moved, skipped))
}
