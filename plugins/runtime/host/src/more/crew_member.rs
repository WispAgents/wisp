//! Subagents who are members of the user's crew (F-crew-11, v1 crew.js `#delegate`): a job of their own through
//! `crew.assign`, and its report read here or on the member's machine.

use std::sync::Arc;

use serde_json::Value;
use wisp_runtime_api::commands::{Delegation, Report};
use wisp_runtime_api::model::{Session, SessionState};
use wisp_runtime_api::{SessionId, TurnId};

use super::delegate::failed;
use super::str_at;
use crate::runtime::Runtime;

impl Runtime {
    /// A crew member's task (v1 crew.js `#delegate`): `crew.assign` with `from` = the lead, so it runs as the
    /// member (their folder, harness, model, kit; on their machine when it's there) and the lead's Team tab lists it.
    pub(super) async fn crew_member(&self, lead: &Session, m: &Value, name: &str, prompt: &str, capped: Option<&Value>) -> Delegation {
        use wisp_crew_api::commands as crew;
        let args = crew::AssignArgs {
            id: str_at(m, "crew"),
            prompt: prompt.to_owned(),
            from: Some(wisp_crew_api::TaskRef { id: lead.id.0.clone(), title: lead.title.clone() }),
            capped: capped.cloned(),
            ..crew::AssignArgs::default()
        };
        let label = Some(str_at(m, "label")).filter(|l| !l.is_empty()).unwrap_or_else(|| name.to_owned());
        let job = match crate::calls::run::<crew::Assign>(&*self.host, args).await {
            Ok(crew::Started { session: Some(job), .. }) => job,
            // a delegated job is never queued; say so rather than report a job that isn't there
            Ok(_) => return failed(name, format!("{label} couldn't start it now. Wait with subagent_results, or do it yourself.")),
            Err(e) if e.code == wisp_kernel::Code::NotFound => {
                return failed(name, format!("{label} is no longer on the user's crew."));
            }
            Err(e) => return failed(name, e.message),
        };
        Delegation {
            id: SessionId(str_at(&job, "id")),
            subagent: name.to_owned(),
            agent: str_at(&job, "agent"),
            model: str_at(&job, "model"),
            effort: str_at(&job, "effort"),
            turn_id: TurnId(String::new()),
            place: Some("crew".into()),
            error: None,
        }
    }

    /// A crew job's report: read here, or on the member's machine (`<machine>|<id>`, nothing brought back: the
    /// job worked in the member's folder there). `None`: it's gone, or not this lead's.
    pub(super) async fn crew_report(self: &Arc<Self>, lead: &Session, key: &str, name: &str) -> Option<Report> {
        if key.contains('|') {
            return self.remote_report(lead, key, false).await.map(|r| Report { subagent: name.to_owned(), ..r });
        }
        let s = self.repo.session(key).await.ok()?;
        let from = s.ext.get(wisp_crew_api::PLUGIN_ID).and_then(|c| c.pointer("/from/id")).and_then(Value::as_str);
        if from != Some(lead.id.as_str()) {
            return None;
        }
        let last = self.last_turn(key).await.ok()?;
        Some(Report {
            id: s.id.clone(),
            subagent: name.to_owned(),
            state: s.state,
            done: s.state != SessionState::Running,
            text: last.as_ref().and_then(|t| t.text.clone()).unwrap_or_default(),
            cost_usd: last.as_ref().and_then(|t| t.stats.as_ref()).and_then(|x| x.cost_usd),
        })
    }
}
