//! Sign in to Claude once for every sandbox (ADR-0047, v1 Settings → Cloud → Sandboxes): `claude setup-token` runs on
//! a pty here, its link shows, the code from that page goes back in, and the long-lived token it prints goes to the
//! grid, sealed per user. Nothing is kept on this computer: the token passes through memory only.
//!
//! `claude setup-token` opens the browser itself and, on this computer, finishes through its own `localhost` callback:
//! no code is ever pasted. The token it then prints is caught as it comes and sent to the grid at once (before, only
//! the paste step read it, so a sign-in finished in the browser never reached the grid and every `/sandbox` still
//! said "Sign in to Claude for sandboxes first"). Once signed in, the launches that waited for it go on.

use std::sync::{Arc, Mutex, PoisonError};

use wisp_kernel::{Call, Ctx, Result, WireError};

use crate::Slot;
use wisp_proc::{ChildHandle, Proc, PtySize, SpawnOpts};
use wisp_sandbox_api::ClaudeSignIn;
use wisp_sandbox_core::{setup_link, setup_token, setup_token_whole};

#[derive(Default)]
pub struct Flow {
    child: Option<ChildHandle>,
    out: String,
    state: ClaudeSignIn,
}

pub type Shared = Arc<Mutex<Flow>>;

fn lock(f: &Shared) -> std::sync::MutexGuard<'_, Flow> {
    f.lock().unwrap_or_else(PoisonError::into_inner)
}

fn state(f: &Shared) -> ClaudeSignIn {
    lock(f).state.clone()
}

pub fn current(f: &Shared) -> ClaudeSignIn {
    let s = state(f);
    if s.state.is_empty() { ClaudeSignIn { state: "idle".into(), ..ClaudeSignIn::default() } } else { s }
}

async fn wait_for(ctx: &Ctx, f: &Shared, secs: u64, done: impl Fn(&Flow) -> bool) -> bool {
    let Ok(clock) = ctx.clock() else { return false };
    let until = clock.now_ms() + i64::try_from(secs * 1000).unwrap_or(i64::MAX);
    loop {
        if done(&lock(f)) {
            return true;
        }
        if clock.now_ms() > until {
            return false;
        }
        clock.sleep(200).await;
    }
}

/// The sign-in's state, and its row (`sandbox.signin` on the launches' topic): Settings and a task waiting for the
/// sign-in both show it as it goes, a sign-in finished in the browser included.
async fn set(f: &Shared, launches: &Slot, s: ClaudeSignIn) {
    lock(f).state = s.clone();
    publish(launches, &s).await;
}

async fn publish(launches: &Slot, s: &ClaudeSignIn) {
    if let Some(l) = launches.get() {
        crate::launches::put_signin(l, s).await;
    }
}

/// Starts `claude setup-token` and waits for its link.
pub async fn begin(ctx: &Ctx, f: &Shared, launches: &Slot) -> Result<ClaudeSignIn> {
    let old = lock(f).child.take();
    if let Some(c) = old {
        let _ = c.kill().await;
    }
    *lock(f) = Flow::default();
    set(f, launches, ClaudeSignIn { state: "waiting".into(), ..ClaudeSignIn::default() }).await;
    let proc = ctx.service::<Proc>()?;
    let bin = std::env::var("WISP_CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    // wide, so the link isn't broken over lines
    let opts = SpawnOpts { pty: Some(PtySize { cols: 400, rows: 40 }), scope: Some(ctx.scope().clone()), ..SpawnOpts::default() };
    let child = proc
        .spawn(&bin, &["setup-token".to_owned()], opts)
        .map_err(|e| WireError::unavailable(format!("Claude Code didn't start ({e}): install it first (Settings → Agents)")))?;
    let (handle, mut output) = child.split();
    lock(f).child = Some(handle);
    let reader = f.clone();
    let (c, launches) = (ctx.clone(), launches.clone());
    ctx.spawn(async move {
        while let Some(o) = output.recv().await {
            let (link, token) = {
                let mut g = lock(&reader);
                g.out.push_str(&String::from_utf8_lossy(&o.data));
                let link = if g.state.link.is_none()
                    && let Some(link) = setup_link(&g.out)
                {
                    g.state = ClaudeSignIn { state: "link".into(), link: Some(link), error: None };
                    Some(g.state.clone())
                } else {
                    None
                };
                // the browser finished it (its localhost callback): the token is there with no code pasted, read
                // once it's printed whole
                (link, if matches!(g.state.state.as_str(), "saving" | "done") { None } else { setup_token_whole(&g.out) })
            };
            if let Some(s) = link {
                publish(&launches, &s).await;
            }
            if let Some(token) = token {
                set(&reader, &launches, ClaudeSignIn { state: "saving".into(), ..ClaudeSignIn::default() }).await;
                save(&c, &c.root_call(), &reader, &token, &launches).await;
                return;
            }
        }
        let ended = matches!(lock(&reader).state.state.as_str(), "waiting" | "link");
        if ended {
            let s =
                ClaudeSignIn { state: "failed".into(), link: None, error: Some("Claude's sign-in ended before it gave a token.".into()) };
            set(&reader, &launches, s).await;
        }
    });
    // its link, or already its end (a browser that finished the sign-in before the link was asked for)
    if !wait_for(ctx, f, 30, |g| g.state.link.is_some() || matches!(g.state.state.as_str(), "saving" | "done" | "failed")).await {
        return Err(WireError::unavailable("Claude's sign-in didn't show its link in 30 s."));
    }
    Ok(state(f))
}

/// The code from the sign-in page: the token Claude prints goes to the grid for every sandbox.
pub async fn code(ctx: &Ctx, call: &Call, f: &Shared, code: &str, launches: &Slot) -> Result<ClaudeSignIn> {
    // finished in the browser meanwhile: nothing to paste any more
    if matches!(lock(f).state.state.as_str(), "saving" | "done") {
        let _ = wait_for(ctx, f, 30, |g| g.state.state != "saving").await;
        return Ok(state(f));
    }
    let handle = lock(f).child.clone().ok_or_else(|| WireError::conflict("Start the sign-in first."))?;
    handle.write(format!("{}\r", code.trim())).map_err(|e| WireError::unavailable(format!("Claude's sign-in: {e}")))?;
    let got =
        wait_for(ctx, f, 60, |g| matches!(g.state.state.as_str(), "saving" | "done" | "failed") || setup_token(&g.out).is_some()).await;
    // the reader caught the token itself (it does, once it's printed whole): its save says how it went
    if matches!(lock(f).state.state.as_str(), "saving" | "done") {
        let _ = wait_for(ctx, f, 30, |g| g.state.state != "saving").await;
        return Ok(state(f));
    }
    let token = setup_token(&lock(f).out);
    let _ = handle.kill().await;
    let Some(token) = token.filter(|_| got) else {
        lock(f).child = None;
        set(f, launches, ClaudeSignIn { state: "failed".into(), link: None, error: Some("That code didn't sign in. Start again.".into()) })
            .await;
        return Ok(state(f));
    };
    set(f, launches, ClaudeSignIn { state: "saving".into(), ..ClaudeSignIn::default() }).await;
    save(ctx, call, f, &token, launches).await;
    Ok(state(f))
}

/// The token to the grid (sealed there for every sandbox), then the launches that waited for it go on.
async fn save(ctx: &Ctx, call: &Call, f: &Shared, token: &str, launches: &Slot) {
    let child = {
        let mut g = lock(f);
        g.out.clear();
        g.child.take()
    };
    if let Some(c) = child {
        let _ = c.kill().await;
    }
    match crate::calls::grid_login(ctx, call, "claude", token).await {
        Ok(()) => {
            set(f, launches, ClaudeSignIn { state: "done".into(), link: None, error: None }).await;
            if let Some(l) = launches.get() {
                crate::launches::signed_in(ctx, l).await;
            }
        }
        Err(e) => {
            tracing::warn!(error = %e.message, "sandbox: the Claude token to the grid");
            let s = ClaudeSignIn {
                state: "failed".into(),
                link: None,
                error: Some(format!("Signed in, but the grid didn't take it: {}", e.message)),
            };
            set(f, launches, s).await;
        }
    }
}
