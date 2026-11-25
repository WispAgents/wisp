// The child that runs wisp's server: the same `wisp` binary as Homebrew and npm (ADR-0013), as `wisp serve`
// with `web/` beside it. It holds the read end of a pipe on its stdin (WISP_EXIT_WITH_STDIN): when this process
// goes, however it goes, that pipe closes and the server shuts down. On Windows the server is also in a Job Object
// that kills it when this process's last handle to the job goes (DESK2-05): Windows has no SIGTERM, and a quit that
// never reached our exit path must not leave the server listening.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const HEALTH_LIMIT: Duration = Duration::from_secs(60);
const QUIT_LIMIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct Server {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Mutex<Option<ChildStdin>>,
    stopping: Arc<AtomicBool>,
    /// Windows: the job the server is in (killed with it when this process ends); held, never used again
    #[cfg(windows)]
    job: Mutex<Option<job::Job>>,
}

// Any HTTP answer counts: a wisp from the CLI, or ours once it's up.
pub fn answers(port: u16, path: &str) -> bool {
    let Ok(addrs) = ("localhost", port).to_socket_addrs() else { return false };
    addrs.into_iter().any(|addr| {
        let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_secs(1)) else { return false };
        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
        let ask = format!("GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n");
        let mut head = [0u8; 5];
        s.write_all(ask.as_bytes()).is_ok() && s.read_exact(&mut head).is_ok() && &head == b"HTTP/"
    })
}

impl Server {
    /// `path`: the login shell's PATH for the server and the agents it starts (src/shell_path.rs); None keeps ours.
    /// `env`: what a shell started from the Dock was never told (src/recorded.rs): the web build and the home.
    pub fn start(
        &self,
        app: &AppHandle,
        wisp: &Path,
        env: &[(&str, PathBuf)],
        data: &Path,
        port: u16,
        path: Option<&str>,
    ) -> std::io::Result<()> {
        let mut cmd = Command::new(wisp);
        cmd.envs(env.iter().map(|(k, v)| (k, v)));
        cmd.arg("serve")
            .current_dir(wisp.parent().unwrap_or(data))
            .env("WISP_PORT", port.to_string())
            .env("WISP_EXIT_WITH_STDIN", "1")
            .env("WISP_DESKTOP", "tauri")
            .env("WISP_DESKTOP_DATA", data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        if let Some(path) = path {
            cmd.env("PATH", path);
        }
        // A release shell has no stderr of its own, so a server that dies young would say nothing.
        if let Ok(log) = std::fs::create_dir_all(data).and_then(|_| std::fs::File::create(data.join("server.log"))) {
            cmd.stderr(log);
        }
        #[cfg(windows)]
        {
            std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
                                                                                          // it joins our job only once it's started (job::Job::kill_on_close): told so, everything it starts stays in
                                                                                          // it however early, and only its update helper leaves (wisp-proc's PARENT_JOB_ENV, DESK2-05)
            cmd.env(job::PARENT_JOB_ENV, "1");
        }
        let mut child = cmd.spawn()?;
        #[cfg(windows)]
        {
            *self.job.lock().unwrap() = job::Job::kill_on_close(&child);
        }
        *self.stdin.lock().unwrap() = child.stdin.take();
        let lines = child.stdout.take().expect("piped stdout");
        *self.child.lock().unwrap() = Some(child);
        let window = app.clone();
        thread::spawn(move || relay(lines, window));
        Ok(())
    }

    /// The login shell's PATH, once it lands behind the start (src/shell_path.rs): the server finds and starts agent
    /// CLIs on it from then on. A line on the stdin pipe, which only this process holds; it waits in the pipe until the
    /// server reads it. No server (it failed to start, or it's a CLI wisp we attached to): nothing to tell.
    pub fn hand_path(&self, path: &str) {
        let Some(line) = path_line(path) else { return };
        if let Some(stdin) = self.stdin.lock().unwrap().as_mut() {
            let _ = stdin.write_all(line.as_bytes()).and_then(|()| stdin.flush());
        }
    }

    // Once it's up the server is what makes this an app: the shell dies with it, and the update flow
    // counts on that (the server exits, a helper waits for the port and opens the new build).
    pub fn exit_with_it(&self, app: AppHandle) {
        let (child, stopping) = (self.child.clone(), self.stopping.clone());
        thread::spawn(move || loop {
            thread::sleep(POLL);
            if stopping.load(Ordering::SeqCst) {
                return;
            }
            let done = child.lock().unwrap().as_mut().is_none_or(|c| !matches!(c.try_wait(), Ok(None)));
            if done {
                return app.exit(0);
            }
        });
    }

    pub fn wait_healthy(&self, port: u16) -> bool {
        let start = Instant::now();
        while start.elapsed() < HEALTH_LIMIT {
            if answers(port, "/api/health") {
                return true;
            }
            let gone = self.child.lock().unwrap().as_mut().is_none_or(|c| !matches!(c.try_wait(), Ok(None)));
            if gone {
                return false;
            }
            thread::sleep(POLL);
        }
        false
    }

    // Blocks the quit until the server is done: closing the pipe is its signal to stop the agents.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        drop(self.stdin.lock().unwrap().take());
        let Some(mut child) = self.child.lock().unwrap().take() else { return };
        let start = Instant::now();
        while start.elapsed() < QUIT_LIMIT {
            if !matches!(child.try_wait(), Ok(None)) {
                return;
            }
            thread::sleep(POLL);
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// `wisp:path <PATH>`, as `wisp serve` reads it (crates/wisp/src/serve.rs `FromShell`); None for what can't be one line.
pub fn path_line(path: &str) -> Option<String> {
    (!path.is_empty() && !path.contains(['\n', '\r', '\0'])).then(|| format!("wisp:path {path}\n"))
}

// The server's log goes on to our stdout. `wisp:shell` lines are for us (a dev server may ask to reload).
fn relay(lines: impl Read, app: AppHandle) {
    for line in BufReader::new(lines).lines().map_while(Result::ok) {
        match line.strip_prefix("wisp:shell ") {
            Some("reload") => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.reload();
                }
            }
            Some("relaunch") => app.request_restart(),
            _ => {
                let _ = writeln!(std::io::stdout(), "{line}"); // println! panics when nobody reads it
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::path_line;

    #[test]
    fn a_path_goes_to_the_server_as_one_line() {
        assert_eq!(path_line("/opt/homebrew/bin:/usr/bin").as_deref(), Some("wisp:path /opt/homebrew/bin:/usr/bin\n"));
        assert_eq!(path_line(""), None);
        assert_eq!(path_line("/a\nwisp:path /evil"), None, "never two lines");
        assert_eq!(path_line("/a\0"), None);
    }
}

/// A Job Object with KILL_ON_JOB_CLOSE (and BREAKAWAY_OK, [`job::LIMITS`]): its processes end when its last handle closes, which is when this process
/// ends, however it ends (a quit, a crash, Task Manager). Best effort: without it the stdin pipe still stops the server.
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// KILL_ON_JOB_CLOSE ends the server with this process, and everything it started. BREAKAWAY_OK lets the one start
    /// that must outlive it leave (`CREATE_BREAKAWAY_FROM_JOB`, wisp-proc's `spawn_detached` with `leave_every_job`):
    /// the update's relaunch helper stops the server, this process quits with it, and the helper must still be there
    /// to run the installer and start the new wisp. wisp-proc asks for breakaway inside this job for that start only.
    /// wisp-proc's `PARENT_JOB_ENV`: the server is in a job of ours (this crate doesn't depend on wisp-proc).
    pub const PARENT_JOB_ENV: &str = "WISP_IN_PARENT_JOB";

    pub const LIMITS: JOB_OBJECT_LIMIT = JOB_OBJECT_LIMIT(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0 | JOB_OBJECT_LIMIT_BREAKAWAY_OK.0);

    pub struct Job(HANDLE);

    // a kernel handle: any thread may hold it
    unsafe impl Send for Job {}

    impl Job {
        pub fn kill_on_close(child: &Child) -> Option<Self> {
            // SAFETY: plain Win32 calls on a job we create and a process handle the Child owns while we use it.
            unsafe {
                let job = CreateJobObjectW(None, None).ok()?;
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = LIMITS;
                let set = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&info).cast(),
                    u32::try_from(std::mem::size_of_val(&info)).unwrap_or(0),
                );
                if set.is_err() || AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())).is_err() {
                    let _ = CloseHandle(job);
                    return None;
                }
                Some(Self(job))
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::System::JobObjects::QueryInformationJobObject;

        /// The job the server runs in lets its outliving children leave: without BREAKAWAY_OK the update's relaunch
        /// helper ended with this process, before `setup.exe /S /R`, and wisp never came back.
        #[test]
        fn the_servers_job_lets_the_relaunch_helper_leave() {
            let mut child = std::process::Command::new("cmd").args(["/d", "/c", "ping -n 5 127.0.0.1 >nul"]).spawn().unwrap();
            let job = Job::kill_on_close(&child).expect("a job");
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            // SAFETY: our open job handle, and a buffer of the size we say
            let read = unsafe {
                QueryInformationJobObject(
                    Some(job.0),
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_mut(&mut info).cast(),
                    u32::try_from(std::mem::size_of_val(&info)).unwrap(),
                    None,
                )
            };
            let _ = child.kill();
            let _ = child.wait();
            read.unwrap();
            let flags = info.BasicLimitInformation.LimitFlags.0;
            assert_ne!(flags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0, 0, "the server still ends with the shell");
            assert_ne!(flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK.0, 0, "what it starts to outlive it may leave");
        }
    }
}
