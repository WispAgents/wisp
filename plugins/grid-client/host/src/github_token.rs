//! A team hub's GitHub App token (ADR-0148 §8e), kept fresh on the hub's own box: every half hour (and at start) it asks
//! the grid for its team's token (`GET /api/github/token`, its box secret) and hands it to gh (`gh auth login
//! --with-token`, on stdin, never on a command line), then makes gh git's credential helper (`gh auth setup-git`), so
//! `gh`, `gh repo clone` and git all reach the repos the team gave the App. A token lasts GitHub's hour; this renews it
//! twice in that. Off a hub, or a team that hasn't connected GitHub, it does nothing (the hub's `GH_TOKEN` secret, if
//! any, is what gh has).
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::net;
use crate::state::St;

/// How often a hub renews its token (GitHub's last an hour).
const EVERY: Duration = Duration::from_secs(30 * 60);
/// After a failed ask: sooner.
const RETRY: Duration = Duration::from_secs(5 * 60);

/// Whether this wisp is a team's hub box (the grid starts it with `WISP_HUB=1`).
pub fn on_hub(env: &dyn Fn(&str) -> Option<String>) -> bool {
    env("WISP_HUB").as_deref() == Some("1") && env("WISP_SANDBOX").as_deref() != Some("1")
}

/// What one ask came to: the token to give gh, nothing to do (no App, not connected), or try again soon.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Token(String),
    Nothing,
    Again,
}

/// The grid's answer to `GET /api/github/token`.
pub fn asked(status: u16, v: &Value) -> Asked {
    match status {
        200 => v.get("token").and_then(Value::as_str).filter(|t| !t.is_empty()).map_or(Asked::Again, |t| Asked::Token(t.to_owned())),
        403 | 404 => Asked::Nothing,
        _ => Asked::Again,
    }
}

async fn gh(st: &St, args: &[&str], input: Option<Vec<u8>>) -> bool {
    let Ok(proc) = st.ctx.service::<wisp_proc::Proc>() else { return false };
    let opts = wisp_proc::ExecOpts {
        input,
        timeout: Some(Duration::from_secs(30)),
        max_output: 16 * 1024,
        scope: Some(st.ctx.scope().clone()),
        ..wisp_proc::ExecOpts::default()
    };
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    match proc.exec("gh", &args, opts).await {
        Ok(r) if r.exit.success() && !r.timed_out => true,
        Ok(r) => {
            tracing::warn!("grid-client: gh {} said: {}", args.first().map_or("", String::as_str), r.stderr_text().trim());
            false
        }
        Err(e) => {
            tracing::warn!("grid-client: gh didn't run: {e}");
            false
        }
    }
}

/// One renewal: what to wait before the next.
async fn renew(st: &Arc<St>, git_set: &mut bool) -> Duration {
    let Some(who) = crate::sandboxes::box_asker(&|k| std::env::var(k).ok()) else { return EVERY };
    let url = format!("{}/api/github/token", who.grid);
    let r = net::call(&st.http, "GET", &url, None, None, &[("authorization", who.auth.as_str())]).await;
    let Ok((status, v)) = r else { return RETRY };
    match asked(status, &v) {
        Asked::Nothing => EVERY,
        Asked::Again => RETRY,
        Asked::Token(t) => {
            if !gh(st, &["auth", "login", "--hostname", "github.com", "--with-token"], Some(t.into_bytes())).await {
                return RETRY;
            }
            if !*git_set {
                *git_set = gh(st, &["auth", "setup-git", "--hostname", "github.com"], None).await;
            }
            tracing::info!("grid-client: the hub's GitHub token renewed");
            EVERY
        }
    }
}

/// On a hub's box: renew its GitHub token now and every half hour, for as long as the plugin runs.
pub fn start(st: &Arc<St>) {
    if !on_hub(&|k| std::env::var(k).ok()) {
        return;
    }
    let st2 = st.clone();
    st.ctx.spawn(async move {
        let mut git_set = false;
        let Ok(clock) = st2.ctx.clock() else { return };
        loop {
            let wait = renew(&st2, &mut git_set).await;
            let ms = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX);
            tokio::select! {
                () = st2.ctx.scope().cancelled() => return,
                () = clock.sleep(ms) => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_a_hub_box_renews_and_the_grids_answer_says_what_to_do() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(a, _)| *a == k).map(|(_, v)| (*v).to_owned())
        };
        assert!(on_hub(&env(&[("WISP_HUB", "1")])));
        assert!(!on_hub(&env(&[("WISP_HUB", "1"), ("WISP_SANDBOX", "1")])), "a hub's sandbox gets its token at its start");
        assert!(!on_hub(&env(&[])));
        assert_eq!(asked(200, &json!({ "token": "ghs_x" })), Asked::Token("ghs_x".into()));
        assert_eq!(asked(200, &json!({})), Asked::Again);
        assert_eq!(asked(404, &json!({ "error": "acme hasn't connected GitHub." })), Asked::Nothing);
        assert_eq!(asked(502, &json!({})), Asked::Again, "GitHub down: sooner");
    }
}
