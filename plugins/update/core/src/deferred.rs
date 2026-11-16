//! Windows npm installs run after wisp has quit. `npm install -g` swaps the whole `wisp-agents` folder, and wisp.exe
//! runs from inside it (`node_modules\@wisp\core-win32-*\bin`): Windows locks a running exe, so npm fails with
//! EPERM/EBUSY, or leaves half a package. (v1 never hit this: node.exe lived outside the package. On macOS and Linux a
//! running binary is an open inode: npm's rename and unlink leave it running, so the install stays where it was.)
//!
//! So on Windows [`crate::Updater::hand_off`]'s `PowerShell` waiter does the install: it stops wisp, waits for it to
//! exit (≤ 30 s), copies wisp.exe aside, runs npm with its output in [`LOG`], writes npm's exit code to [`EXIT`] and
//! starts wisp again with the same args: the new copy, or the old one when npm failed (npm rolls its own changes
//! back; if wisp.exe is gone anyway, the copy set aside). The started wisp reads [`PENDING`] (written before the
//! hand-off), [`EXIT`] and [`LOG`] through [`outcome`], and its update row says what happened.

use serde::{Deserialize, Serialize};

use crate::install::Platform;
use crate::updater::run_failed;
use crate::version::newer;

/// What the install was meant to do, written by the host before the hand-off.
pub const PENDING: &str = "update-pending.json";
/// npm's exit code (or `busy`: wisp didn't stop), written by the waiter.
pub const EXIT: &str = "update-exit.txt";
/// npm's output.
pub const LOG: &str = "update-npm.log";
/// wisp.exe as it was, for when npm fails and takes it with it.
pub const BACKUP: &str = "wisp-old.exe";
/// How long the waiter waits for wisp to exit before it gives up and installs nothing.
pub const WAIT_S: u32 = 30;

/// Where the waiter and the next start meet: the plugin's own folder, and this wisp's binary (paths in the target's
/// form).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    pub dir: String,
    pub exe: String,
}

impl Files {
    pub fn path(&self, platform: &Platform, name: &str) -> String {
        platform.join(&self.dir, &[name])
    }
}

/// [`PENDING`]'s body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// the version that handed off
    pub from: String,
    /// the version on offer
    pub to: String,
    /// what the waiter ran (`npm install -g wisp-agents@<tag>`)
    pub cmd: String,
}

/// What the started wisp says about the install the waiter ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Updated { from: String, to: String },
    Failed(String),
}

/// After a hand-off: `exit` is [`EXIT`]'s text (None: missing), `log` [`LOG`]'s, `current` this wisp's version.
pub fn outcome(p: &Pending, exit: Option<&str>, log: &str, current: &str) -> Outcome {
    let cmd = &p.cmd;
    match exit.map(str::trim) {
        // the waiter never got to write it: it was stopped, or wisp was started by hand before it finished
        None => Outcome::Failed(format!("The update to {} didn't finish. Run `{cmd}` yourself, then start wisp.", p.to)),
        Some("busy") => Outcome::Failed(format!("wisp didn't stop within {WAIT_S} seconds, so `{cmd}` didn't run. Try again.")),
        Some("start") => Outcome::Failed(format!("The update couldn't start `{cmd}` (cmd.exe didn't run). Run it yourself.")),
        Some("0") if newer(current, &p.from) => Outcome::Updated { from: p.from.clone(), to: current.to_owned() },
        Some("0") => Outcome::Failed(format!("npm installed wisp somewhere else: this copy is still {current}.")),
        Some(code) => {
            let out = if log.trim().is_empty() { format!("exit code {code}") } else { log.to_owned() };
            Outcome::Failed(run_failed(cmd, &out))
        }
    }
}

/// A `PowerShell` single-quoted string: nothing expands in it, a quote is doubled ([`ps_escape`]).
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", ps_escape(s))
}

/// The inside of a `PowerShell` single-quoted string. `PowerShell` ends one at `'` and at the curly quotes too
/// (‘ ’ ‚ ‛, as in `C:\Users\Ann O’Brien`), so each is doubled.
pub fn ps_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
            out.push(c);
        }
        out.push(c);
    }
    out
}

/// The waiter's script on Windows when npm runs after wisp exits. `relaunch`: the command that starts wisp again
/// (through `cmd /S /C`); `args`: its wisp arguments, for the copy set aside.
pub fn script(pid: u32, npm: &str, relaunch: &str, args: &str, files: &Files, platform: &Platform) -> String {
    let exe = ps_quote(&files.exe);
    let exit = ps_quote(&files.path(platform, EXIT));
    let backup = ps_quote(&files.path(platform, BACKUP));
    // cmd /S /C "<line>": cmd drops the outer quotes and runs the rest as typed, inner quotes and all
    let install = ps_quote(&format!("/d /s /c \"{npm} > \"{}\" 2>&1\"", files.path(platform, LOG)));
    let restart = ps_quote(&format!("/S /C \"{relaunch}\""));
    let own = ps_quote(args.trim());
    [
        "$ErrorActionPreference = 'Continue'".to_owned(),
        "Start-Sleep -Seconds 1".to_owned(),
        format!("Stop-Process -Id {pid} -Force -ErrorAction SilentlyContinue"),
        format!("Wait-Process -Id {pid} -Timeout {WAIT_S} -ErrorAction SilentlyContinue"),
        format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ Set-Content -LiteralPath {exit} -Value 'busy'; exit 1 }}"),
        format!("Copy-Item -LiteralPath {exe} -Destination {backup} -Force -ErrorAction SilentlyContinue"),
        format!(
            "try {{ $p = Start-Process -FilePath cmd.exe -ArgumentList {install} -Wait -PassThru -WindowStyle Hidden -ErrorAction Stop; \
             $code = $p.ExitCode }} catch {{ $code = 'start' }}"
        ),
        format!("Set-Content -LiteralPath {exit} -Value $code"),
        format!(
            "if (Test-Path -LiteralPath {exe}) {{ Remove-Item -LiteralPath {backup} -Force -ErrorAction SilentlyContinue; \
             Start-Process -FilePath cmd.exe -ArgumentList {restart} -WindowStyle Hidden }} \
             else {{ Start-Process -FilePath {backup} -ArgumentList {own} -WindowStyle Hidden }}"
        ),
    ]
    .join("\n")
}
