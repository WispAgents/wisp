//! `runtime.defaultModel` (owner's delegate 2026-11-18): what a harness's Default runs in a folder, as the harness reads
//! its own config there (a project that pins a model, else this computer's), never a run. The composer's chip names it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;

use common::{Fake, rig_on};
use wisp_kernel::BoxFuture;
use wisp_runtime::Recorder;
use wisp_runtime_api::Harness;
use wisp_runtime_api::commands::DefaultModelArgs;
use wisp_runtime_api::harness::{HarnessFound, HarnessInfo, HarnessRun, HarnessSys, Launch, RunDriver};

/// A harness whose Default runs Opus in /code/app and Sonnet anywhere else.
struct Pinned {
    inner: Arc<dyn Harness>,
}

impl Harness for Pinned {
    fn info(&self) -> &HarnessInfo {
        self.inner.info()
    }
    fn detect<'a>(&'a self, sys: &'a dyn HarnessSys) -> BoxFuture<'a, HarnessFound> {
        self.inner.detect(sys)
    }
    fn launch(&self, run: &HarnessRun) -> wisp_kernel::Result<Launch> {
        self.inner.launch(run)
    }
    fn driver(&self, run: &HarnessRun) -> Box<dyn RunDriver> {
        self.inner.driver(run)
    }
    fn default_model<'a>(&'a self, _sys: &'a dyn HarnessSys, cwd: Option<&'a str>) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move { Some(if cwd == Some("/code/app") { "Opus" } else { "Sonnet" }.to_owned()) })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_default_is_named_for_the_folder_it_runs_in() {
    let pinned: Arc<dyn Harness> = Arc::new(Pinned { inner: Fake::named("pinned", "Pinned", &[]) });
    let r = rig_on(Recorder::default(), vec![pinned, Fake::named("plain", "Plain", &[])]).await;
    let ask = |agent: &str, cwd: Option<&str>| DefaultModelArgs { agent: agent.into(), cwd: cwd.map(str::to_owned) };
    assert_eq!(r.rt.default_model(&ask("pinned", Some("/code/app"))).await.as_deref(), Some("Opus"));
    assert_eq!(r.rt.default_model(&ask("pinned", Some("/code/other"))).await.as_deref(), Some("Sonnet"));
    assert_eq!(r.rt.default_model(&ask("pinned", None)).await.as_deref(), Some("Sonnet"));
    // a harness that can't tell, and one that isn't there: nothing (the chip keeps what detect said, else Default)
    assert_eq!(r.rt.default_model(&ask("plain", Some("/code/app"))).await, None);
    assert_eq!(r.rt.default_model(&ask("nope", None)).await, None);
}
