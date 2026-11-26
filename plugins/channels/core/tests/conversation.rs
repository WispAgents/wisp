//! v1 conversation.js through a stand-in channel and phone: a message is heard (triggers on `chat.message`,
//! ADR-0043), read by the grammar, and does what it asks: a task, a list, a refusal, a follow-up in its thread.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unnecessary_literal_bound)]

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{Value, json};
use wisp_channels_api::conversation::Conversation;
use wisp_channels_api::{
    ActArgs, ChannelCtx, Later, Mode, Parsed, Phone, ProjectView, RunOpts, SessionView, StartOpts, Started, Thread, ThreadCtx,
};
use wisp_channels_core::conversation::{converse, with_about};
use wisp_kernel::Result;

#[derive(Default)]
struct FakePhone {
    log: Mutex<Vec<String>>,
}

impl FakePhone {
    fn note(&self, s: String) {
        self.log.lock().unwrap().push(s);
    }
    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

#[async_trait]
impl Phone for FakePhone {
    async fn context(&self) -> Result<ChannelCtx> {
        Ok(ChannelCtx::default())
    }
    fn count(&self, kind: &str, detail: Option<&str>) {
        self.note(format!("count {kind} {}", detail.unwrap_or_default()));
    }
    fn on_grid(&self) -> bool {
        false
    }
    async fn grid_reply(&self, _t: &str, _x: &str) -> Result<()> {
        Ok(())
    }
    fn thread_saved(&self) {}
    fn heard(&self, channel: &str, sender: &str, text: &str) {
        self.note(format!("heard {channel} {sender} {text}"));
    }
    async fn thread_ctx(&self, _s: &str) -> Result<ThreadCtx> {
        Ok(ThreadCtx { agent: "claude".into(), machine: String::new() })
    }
    async fn start_parsed(&self, p: &Parsed, o: &StartOpts) -> Result<Started> {
        self.note(format!("start {} in {} cap {:?}", p.prompt, p.project.as_ref().map_or("-", |x| x.name.as_str()), o.cap));
        let session = Some(SessionView { id: "t9".into(), ..SessionView::default() });
        Ok(Started { ack: "▶ started".into(), session, ..Started::default() })
    }
    async fn stop(&self, s: &str) -> Result<String> {
        self.note(format!("stop {s}"));
        Ok("Stopped.".into())
    }
    async fn status(&self, _s: &str) -> Result<String> {
        Ok("running".into())
    }
    async fn run(&self, s: &str, p: &Parsed, _o: &RunOpts, _l: Option<Later>) -> Result<String> {
        self.note(format!("run {s} {}", p.prompt));
        Ok("✓ sent".into())
    }
    async fn mention(&self, _s: &str, p: &Parsed, o: &StartOpts) -> Result<Started> {
        self.start_parsed(p, o).await
    }
    async fn launch(&self, _p: &ProjectView, _x: &str) -> Result<Option<SessionView>> {
        Ok(None)
    }
    async fn act(&self, _a: &ActArgs) -> Result<wisp_channels_api::Acted> {
        Ok(wisp_channels_api::Acted::done("Marked done."))
    }
    async fn projects(&self) -> Result<Vec<ProjectView>> {
        Ok(Vec::new())
    }
    async fn session(&self, _id: &str) -> Result<Option<SessionView>> {
        Ok(None)
    }
    async fn sessions(&self) -> Result<Vec<SessionView>> {
        Ok(Vec::new())
    }
}

struct Chat<'a> {
    phone: &'a FakePhone,
    ctx: ChannelCtx,
    said: Mutex<Vec<String>>,
    /// a channel with threads (Slack, GitHub): what `acked` was handed
    threaded: bool,
    acked: Mutex<Vec<Option<String>>>,
}

#[async_trait]
impl Conversation for Chat<'_> {
    fn phone(&self) -> &dyn Phone {
        self.phone
    }
    fn ctx(&self) -> &ChannelCtx {
        &self.ctx
    }
    fn channel(&self) -> &str {
        "test"
    }
    fn name(&self) -> &str {
        "Ana"
    }
    fn sender(&self) -> &str {
        "u1"
    }
    fn cap(&self) -> Mode {
        Mode::Edits
    }
    fn approvals(&self) -> bool {
        false
    }
    fn no_thread(&self) -> &str {
        "Reply to the task’s message to do that."
    }
    async fn say(&self, text: String) -> Result<Value> {
        self.said.lock().unwrap().push(text);
        Ok(json!({}))
    }
    async fn thread(&self, _p: &Parsed) -> Option<Thread> {
        self.threaded.then(|| Thread { channel: "test".into(), reference: Some("r1".into()), ..Thread::default() })
    }
    fn acked(&self, thread: Option<&Thread>, _sent: &Value) {
        if let Some(t) = thread {
            self.acked.lock().unwrap().push(t.session.clone());
        }
    }
}

fn chat(phone: &FakePhone) -> Chat<'_> {
    let api = ProjectView { id: "p1".into(), name: "api".into(), path: "/work/api".into(), ..ProjectView::default() };
    Chat {
        phone,
        ctx: ChannelCtx { projects: vec![api.clone()], default_project: Some(api), ..ChannelCtx::default() },
        said: Mutex::default(),
        threaded: false,
        acked: Mutex::default(),
    }
}

#[tokio::test]
async fn a_task_started_in_a_thread_is_linked_to_it_and_runs_in_the_places_own_project() {
    let phone = FakePhone::default();
    let web = ProjectView { id: "p2".into(), name: "web".into(), path: "/work/web".into(), ..ProjectView::default() };
    // a GitHub comment's conversation: its repo's project is the context's default
    let c = Chat {
        threaded: true,
        ctx: ChannelCtx {
            default_project: Some(web.clone()),
            projects: vec![web, chat(&phone).ctx.projects[0].clone()],
            ..ChannelCtx::default()
        },
        ..chat(&phone)
    };
    converse(&c, "review this PR", None, None).await.unwrap();
    assert!(phone.log().contains(&"start review this PR in web cap Some(Edits)".to_owned()), "{:?}", phone.log());
    // the thread is made before the task exists: acked gets it with the task, so its report comes back there
    assert_eq!(c.acked.lock().unwrap().as_slice(), [Some("t9".to_owned())]);
}

#[tokio::test]
async fn a_message_through_the_door_is_heard_by_triggers_and_starts_its_task() {
    let phone = FakePhone::default();
    let c = chat(&phone);
    converse(&c, "in api plan: fix the login", None, None).await.unwrap();
    let log = phone.log();
    assert_eq!(log[0], "heard test u1 in api plan: fix the login", "chat.message hears it first, as typed");
    assert_eq!(log[1], "start fix the login in api cap Some(Edits)");
    assert_eq!(c.said.lock().unwrap().as_slice(), ["▶ started"]);
}

#[tokio::test]
async fn a_list_a_refusal_and_full_auto_past_the_cap_start_nothing() {
    let phone = FakePhone::default();
    let c = chat(&phone);
    converse(&c, "projects", None, None).await.unwrap();
    converse(&c, "in nowhere: fix it", None, None).await.unwrap();
    converse(&c, "full auto: deploy it", None, None).await.unwrap();
    let said = c.said.lock().unwrap().clone();
    assert!(said[0].starts_with("Projects:\n• api  ← default"), "{}", said[0]);
    assert!(
        said[1].starts_with("No project “nowhere”.") && said[1].ends_with("Nothing started. Start with “:” to send it as written."),
        "{}",
        said[1]
    );
    assert!(said[2].ends_with("Nothing started."), "past this chat's cap: {}", said[2]);
    assert!(!phone.log().iter().any(|l| l.starts_with("start ")), "{:?}", phone.log());
    assert!(phone.log().contains(&"count list projects".to_owned()) && phone.log().contains(&"count refused no-match".to_owned()));
}

#[tokio::test]
async fn in_a_tasks_thread_a_message_follows_up_and_stop_stops_it() {
    let phone = FakePhone::default();
    let c = chat(&phone);
    converse(&c, "also fix the signup", Some("t1"), None).await.unwrap();
    converse(&c, "stop", Some("t1"), None).await.unwrap();
    converse(&c, "stop", None, None).await.unwrap();
    let log = phone.log();
    assert!(log.contains(&"run t1 also fix the signup".to_owned()) && log.contains(&"stop t1".to_owned()), "{log:?}");
    assert_eq!(c.said.lock().unwrap().last().unwrap(), "Reply to the task’s message to do that.", "stop out of a thread");
}

#[test]
fn a_tasks_prompt_says_where_it_was_asked_after_the_message() {
    let about = "GitHub PR me/api#6: https://github.com/me/api/pull/6\nRead it with `gh pr view 6 --repo me/api`.";
    let p = with_about("review this PR", Some(about));
    assert!(p.starts_with("review this PR\n\n---\nGitHub PR me/api#6"), "the message first: the title stays its: {p}");
    assert!(p.ends_with("--repo me/api`."), "{p}");
    assert_eq!(with_about("fix it", None), "fix it", "a chat with no place says nothing more");
    assert_eq!(with_about("fix it", Some("  \n")), "fix it");
}
