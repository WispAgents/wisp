//! A child on a pseudo-terminal (portable-pty), replacing v1's Python `pty.fork` helper and its
//! `script(1)` fallback. portable-pty is blocking, so each terminal has three small threads: one reads
//! the terminal into the output channel, one writes typed input (a program that stops reading blocks
//! only that thread: a big paste never hangs the printer), one waits for the exit.
//!
//! The child is a session leader (setsid), so its pid is its process group and a tree kill reaches
//! everything it started; on Windows it goes into a Job Object like a piped child.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use crate::BoxFuture;
use portable_pty::{CommandBuilder, MasterPty, PtySize as Size};
use tokio::sync::{mpsc, watch};

use crate::child::{ChildControl, ChildHandle};
use crate::pipes::{DRAIN, kill_tree, signal_tree};
use crate::resolve::Resolved;
use crate::tree::Tree;
use crate::{Child, Exit, KillReport, Output, ProcError, PtySize, Result, Signal, SpawnOpts, Stream};

const CHUNK: usize = 64 * 1024;
/// ^D: a terminal's end of input.
const EOT: u8 = 0x04;

/// The output sender, shared so the wait thread can close the channel after the drain even while a
/// background process keeps the terminal open (and the reader blocked in `read`).
type Slot = Arc<Mutex<Option<mpsc::Sender<Output>>>>;

pub(crate) fn spawn(bin: &str, r: &Resolved, opts: &SpawnOpts, size: PtySize) -> Result<Child> {
    // The size goes on when the PTY is made, before the program starts: v1 learned that a size set
    // afterwards races the program's first look (`stty size` said 0 0 on a busy machine).
    let size = Size { rows: size.rows.max(1), cols: size.cols.max(1), pixel_width: 0, pixel_height: 0 };
    let pair = portable_pty::native_pty_system().openpty(size).map_err(|e| ProcError::spawn(bin, e))?;
    let child = pair.slave.spawn_command(command(r, opts)).map_err(|e| ProcError::spawn(bin, e))?;
    // Our copy of the slave side would keep the terminal open after the program exits.
    drop(pair.slave);
    let pid = child.process_id().ok_or_else(|| ProcError::spawn(bin, "no pid"))?;
    let tree = Tree::new(pid);
    let reader = pair.master.try_clone_reader().map_err(|e| ProcError::spawn(bin, e))?;
    let writer = pair.master.take_writer().map_err(|e| ProcError::spawn(bin, e))?;
    let (out_tx, out_rx) = crate::output_channel();
    let slot: Slot = Arc::new(Mutex::new(Some(out_tx)));
    let (done_tx, done_rx) = std_mpsc::channel();
    let (in_tx, in_rx) = std_mpsc::channel::<Vec<u8>>();
    let alive = Arc::new(AtomicBool::new(true));
    let (exit_tx, exit_rx) = watch::channel(None);
    let name = format!("pty-{pid}");
    let reader_slot = slot.clone();
    // Windows: ConPTY (portable-pty opens it with INHERIT_CURSOR) asks for the cursor first and holds the program back
    // until it hears an answer; wisp is that terminal whether or not a screen is attached (see `answer_startup_query`)
    let answer = cfg!(windows).then(|| in_tx.clone());
    thread::Builder::new().name(format!("{name}-read")).spawn(move || read(reader, &reader_slot, &done_tx, answer))?;
    thread::Builder::new().name(format!("{name}-write")).spawn(move || feed(&in_rx, writer))?;
    let alive2 = alive.clone();
    // Listed with the guard once nothing but the wait thread can fail; that thread unlists it after the reap.
    #[cfg(unix)]
    crate::guard::watch(pid);
    let waiter = thread::Builder::new().name(format!("{name}-wait")).spawn(move || {
        let exit = wait(child);
        alive2.store(false, Ordering::SeqCst);
        #[cfg(unix)]
        crate::guard::reaped(pid);
        // Output first, then the exit: the last screen includes the final lines.
        let _ = done_rx.recv_timeout(DRAIN);
        slot.lock().unwrap_or_else(PoisonError::into_inner).take();
        tracing::debug!(pid, code = ?exit.code, signal = ?exit.signal, "pty exit");
        let _ = exit_tx.send(Some(exit));
    });
    if let Err(e) = waiter {
        #[cfg(unix)]
        crate::guard::reaped(pid);
        return Err(e.into());
    }
    let control = PtyControl { input: Mutex::new(Some(in_tx)), master: Mutex::new(pair.master), tree, alive };
    let handle = ChildHandle::new(pid, true, opts.grace.unwrap_or(crate::KILL_GRACE), Arc::new(control), exit_rx);
    Ok(Child { handle, output: out_rx })
}

/// The variable a `cmd /c` line is handed over in (see [`cmd_line_by_env`]).
const CMD_LINE_VAR: &str = "WISP_CMD_LINE";

/// Windows: a `cmd /c <line>` holding a `"` for `ConPTY`, as `/c %WISP_CMD_LINE%` with the line in that variable.
/// portable-pty has no raw command line: it quotes every argument for `CommandLineToArgvW`, so a `"` in the line
/// arrives as `\"`, which cmd.exe reads as a backslash and a quote (a run script's `echo "hi"`, and every unknown
/// batch file wisp starts through cmd.exe with its arguments already quoted for it, `verbatim`). cmd expands the
/// variable once, then parses the line as if it had been typed after `/c`; a verbatim `/s /c "<line>"` loses the outer
/// quotes `/s` would strip. That one expansion is also why a line without a `"` is left alone: its own `%PATH%` stays
/// expanded (portable-pty's quotes around it are the ones `/s` strips). A line with both loses its `%X%` expansion.
/// `None` when it isn't cmd.exe with one such line after `/c`.
pub(crate) fn cmd_line_by_env(command: &str, args: &[String], verbatim: bool) -> Option<(Vec<String>, String)> {
    let name = command.rsplit(['\\', '/']).next().unwrap_or(command).to_ascii_lowercase();
    if name != "cmd.exe" && name != "cmd" {
        return None;
    }
    let at = args.iter().position(|a| a.eq_ignore_ascii_case("/c"))?;
    let [line] = &args[at + 1..] else { return None };
    let strip = verbatim && args[..at].iter().any(|a| a.eq_ignore_ascii_case("/s"));
    let line = match line.strip_prefix('"').and_then(|l| l.strip_suffix('"')) {
        Some(inner) if strip => inner,
        _ => line.as_str(),
    };
    if line.is_empty() || !line.contains('"') {
        return None;
    }
    let mut out = args[..=at].to_vec();
    out.push(format!("%{CMD_LINE_VAR}%"));
    Some((out, line.to_owned()))
}

fn command(r: &Resolved, opts: &SpawnOpts) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(&r.command);
    let by_env = if cfg!(windows) { cmd_line_by_env(&r.command, &r.args, r.verbatim) } else { None };
    match &by_env {
        Some((args, _)) => cmd.args(args),
        None => cmd.args(&r.args),
    }
    // portable-pty starts in $HOME without one; a piped child starts where wisp is: keep them alike.
    let cwd = opts.cwd.clone().or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| PathBuf::from("/"));
    cmd.cwd(cwd);
    if opts.env.clear {
        cmd.env_clear();
    }
    for k in &opts.env.remove {
        cmd.env_remove(k);
    }
    for (k, v) in &opts.env.set {
        cmd.env(k, v);
    }
    if let Some((_, line)) = by_env {
        cmd.env(CMD_LINE_VAR, line);
    }
    cmd
}

/// A terminal's cursor-position query (DSR 6), and the answer wisp gives `ConPTY`'s (row 1, column 1).
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_AT_HOME: &[u8] = b"\x1b[1;1R";
/// How much of the start of the output is looked at for `ConPTY`'s own query; it is the first thing `ConPTY` writes.
const STARTUP_LOOK: usize = 4096;

/// `ConPTY`'s start-up query, taken out of `data` (the start of the output) when it is there: true when it was, and
/// then wisp answers it. Opened with `PSEUDOCONSOLE_INHERIT_CURSOR`, `ConPTY` writes `ESC[6n` before the program runs
/// and holds everything back until it hears `ESC[row;colR`: with no screen attached (a run script, a sandbox's
/// harness, a task terminal nobody opened) the program never ran. It is `ConPTY`'s, not the program's, so it never
/// reaches a screen either: one that is attached (`xterm.js` answers DSR 6) would answer it a second time, and that
/// answer would be typed into the program. Later queries are the program's and pass through.
pub(crate) fn answer_startup_query(data: &mut Vec<u8>) -> bool {
    let Some(at) = data.windows(CURSOR_QUERY.len()).position(|w| w == CURSOR_QUERY) else { return false };
    data.drain(at..at + CURSOR_QUERY.len());
    true
}

fn read(mut reader: Box<dyn Read + Send>, slot: &Slot, done: &std_mpsc::Sender<()>, mut answer: Option<std_mpsc::Sender<Vec<u8>>>) {
    let mut buf = vec![0u8; CHUNK];
    let mut seen = 0usize;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut data = buf[..n].to_vec();
                if let Some(input) = answer.take() {
                    seen += n;
                    if answer_startup_query(&mut data) {
                        let _ = input.send(CURSOR_AT_HOME.to_vec());
                    } else if seen < STARTUP_LOOK {
                        answer = Some(input);
                    }
                    if data.is_empty() {
                        continue;
                    }
                }
                // Cloned out of the lock: a full channel blocks this thread, never the wait thread.
                let tx = slot.lock().unwrap_or_else(PoisonError::into_inner).clone();
                match tx {
                    Some(tx) => {
                        let _ = tx.blocking_send(Output { stream: Stream::Stdout, data });
                    }
                    None => break, // the exit was announced: it is the last word
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            // EIO is how Linux reports a terminal whose program has gone.
            Err(_) => break,
        }
    }
    let _ = done.send(());
}

fn feed(rx: &std_mpsc::Receiver<Vec<u8>>, mut writer: Box<dyn Write + Send>) {
    while let Ok(bytes) = rx.recv() {
        if writer.write_all(&bytes).and_then(|()| writer.flush()).is_err() {
            break;
        }
    }
}

/// portable-pty reports a signal as strsignal(3) text; the std status underneath has the number.
fn wait(child: Box<dyn portable_pty::Child + Send + Sync>) -> Exit {
    let child: Box<dyn portable_pty::Child> = child;
    match child.downcast::<std::process::Child>() {
        Ok(mut c) => c.wait().map(crate::pipes::exit_of).unwrap_or_default(),
        Err(mut other) => other.wait().map(|s| Exit { code: i32::try_from(s.exit_code()).ok(), signal: None }).unwrap_or_default(),
    }
}

struct PtyControl {
    input: Mutex<Option<std_mpsc::Sender<Vec<u8>>>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    tree: Tree,
    alive: Arc<AtomicBool>,
}

impl ChildControl for PtyControl {
    fn write(&self, data: Vec<u8>) -> Result<()> {
        let guard = self.input.lock().unwrap_or_else(PoisonError::into_inner);
        guard.as_ref().ok_or(ProcError::StdinClosed)?.send(data).map_err(|_| ProcError::Exited)
    }
    fn close_stdin(&self) {
        let _ = self.write(vec![EOT]);
    }
    fn resize(&self, size: PtySize) -> Result<()> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(ProcError::Exited);
        }
        let size = Size { rows: size.rows.max(1), cols: size.cols.max(1), pixel_width: 0, pixel_height: 0 };
        self.master.lock().unwrap_or_else(PoisonError::into_inner).resize(size).map_err(|e| ProcError::Io(io::Error::other(e.to_string())))
    }
    fn signal(&self, sig: Signal, whole: bool) -> Result<()> {
        signal_tree(&self.tree, &self.alive, sig, whole)
    }
    fn kill(&self, sig: Signal, grace: Duration) -> BoxFuture<'static, KillReport> {
        kill_tree(self.tree.clone(), self.alive.load(Ordering::SeqCst), sig, grace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ConPTY`'s first write (Windows-shaped: the query, then the screen it starts with) loses the query; the rest
    /// is kept as it was, and a chunk without one is left alone (the program's own later queries pass through).
    #[test]
    fn conpty_s_start_up_cursor_query_is_answered_and_kept_off_the_screen() {
        let mut first = b"\x1b[6n\x1b[?9001h\x1b[?1004h".to_vec();
        assert!(answer_startup_query(&mut first));
        assert_eq!(first, b"\x1b[?9001h\x1b[?1004h");
        let mut only = CURSOR_QUERY.to_vec();
        assert!(answer_startup_query(&mut only));
        assert!(only.is_empty(), "nothing left to show");
        let mut plain = b"serving\r\n".to_vec();
        assert!(!answer_startup_query(&mut plain));
        assert_eq!(plain, b"serving\r\n");
        let mut half = b"\x1b[6".to_vec();
        assert!(!answer_startup_query(&mut half), "not a query yet");
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    /// A run script with quotes, and an unknown batch file as `resolve` hands it over (verbatim, `/s /c "<line>"`):
    /// the line goes in the variable as cmd.exe should read it, and the arguments portable-pty quotes have no `"`.
    #[test]
    fn a_cmd_line_with_quotes_reaches_conpty_unquoted_through_a_variable() {
        let (args, line) =
            cmd_line_by_env("C:\\Windows\\System32\\cmd.exe", &s(&["/d", "/s", "/c", "echo \"hi\" > ran.txt && npm run dev"]), false)
                .unwrap();
        assert_eq!(args, s(&["/d", "/s", "/c", "%WISP_CMD_LINE%"]));
        assert_eq!(line, "echo \"hi\" > ran.txt && npm run dev", "as written: /s strips nothing that isn't verbatim");
        let quoted = "\"C:\\Program Files\\t\\run.cmd ^^^\"fix it^^^\"\"";
        let (args, line) = cmd_line_by_env("C:\\WINDOWS\\system32\\CMD.EXE", &s(&["/d", "/s", "/c", quoted]), true).unwrap();
        assert!(args.iter().all(|a| !a.contains('"')), "{args:?}");
        assert_eq!(line, "C:\\Program Files\\t\\run.cmd ^^^\"fix it^^^\"", "the outer quotes /s would strip");
        assert_eq!(cmd_line_by_env("C:\\x\\claude.exe", &s(&["/c", "x"]), false), None, "not cmd.exe");
        assert_eq!(cmd_line_by_env("cmd.exe", &s(&["/d", "/c", "a", "b"]), false), None, "more than one line: left as it is");
        assert_eq!(cmd_line_by_env("cmd.exe", &s(&["/k"]), false), None, "an interactive cmd");
        let plain = s(&["/d", "/s", "/c", "echo %USERPROFILE% > ran.txt"]);
        assert_eq!(cmd_line_by_env("cmd.exe", &plain, false), None, "no quote: left as it is, so its %USERPROFILE% expands");
    }
}
