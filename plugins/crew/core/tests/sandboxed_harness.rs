//! A member whose jobs run in a sandbox may run any harness (ADR-0148): Claude with the grid's sign-in for sandboxes,
//! the others signing in inside the sandbox or needing none (a team hub's triager on opencode's free models). Before, a
//! sandboxed member had to be Claude Code, though wisp.sandbox itself let the others start (`may_start`).
#![allow(clippy::unwrap_used)]

use wisp_crew_api::commands::MemberBody;
use wisp_crew_core::cron::CronOnly;
use wisp_crew_core::kit::Ids;
use wisp_crew_core::member::{Checks, normalize_member};

struct Seq(u32);
impl Ids for Seq {
    fn next_id(&mut self) -> String {
        self.0 += 1;
        format!("{:016x}", self.0)
    }
}

#[test]
fn a_sandboxed_member_runs_any_harness() {
    let yes = |_: &str| true;
    let no = |_: &str| false;
    let c = Checks { taken: &[], exists: &yes, machine_ok: &no, words: &CronOnly, agent_ok: &no, default_agent: "claude" };
    for agent in ["opencode", "codex", "claude"] {
        let body = MemberBody {
            name: Some(format!("T-{agent}")),
            agent: Some(agent.into()),
            sandbox: Some("job".into()),
            ..MemberBody::default()
        };
        let m = normalize_member(&body, None, &c, &mut Seq(0), 1).unwrap();
        assert_eq!((m.agent.as_str(), m.sandbox.as_str()), (agent, "job"));
    }
}
