//! wisp.runtime's other resource kinds (ADR-0028): projects, harnesses, teams and skills, kept as declared
//! (`StoredKind`) and acted on when a task starts (`specs::prepare`):
//! - a **project** whose path holds the task's folder gives its agent (when the task names none), its
//!   worktree default, runs its setup commands first, and its instructions go in the system prompt;
//! - a **harness** that is off refuses the task; its model and effort fill what the task left out;
//! - a **team** the task names becomes its subagents;
//! - the **skills** its agent profile names go in the system prompt.
//!
//! One module per kind; this one holds the stored kinds and registers them.

mod brief;
mod harness;
mod issues;
mod project;
mod project_repo;
mod team;

use wisp_kernel::Setup;
use wisp_resources_api::{KindInfo, ResourceKind, StoredKind, stored};

pub use brief::Brief;
pub use harness::{HarnessSpec, declared};
pub use issues::IssueProjects;
pub use project::{ProjectSpec, project_add, project_for, project_path, with_test};
pub use team::{SkillSpec, TeamSpec};

/// The stored kinds, for `prepare` to read.
#[derive(Debug)]
pub struct Kinds {
    pub project: std::sync::Arc<wisp_resources_api::StoredKind>,
    pub harness: std::sync::Arc<wisp_resources_api::StoredKind>,
    pub skill: std::sync::Arc<wisp_resources_api::StoredKind>,
    pub team: std::sync::Arc<wisp_resources_api::StoredKind>,
}

pub fn register(s: &mut Setup) -> std::sync::Arc<Kinds> {
    let owner = "wisp.runtime";
    std::sync::Arc::new(Kinds {
        // every screen shows projects (chips, New task's "in", Home): published as `project` on `config`
        // its path canonical whoever applies it (`wisp add .` sent "/repo/.", a pasted "C:\\x\\" or "\\\\?\\C:\\x")
        // one project per folder (setup audit F23: a formula's second name for a folder was a second project)
        // a folder that's there (setup audit B2: `wisp import` and `wisp apply` let a missing one in)
        project: project_kind(s, owner),
        harness: harness::harness_kind(s),
        skill: stored::<SkillSpec>(s, |sc| KindInfo::new("skill", "skills", owner, sc).order(15)),
        team: stored::<TeamSpec>(s, |sc| {
            KindInfo::new("team", "teams", owner, sc)
                .order(25)
                .names(wisp_resources_api::Names::new("lead", "agent"))
                .names(wisp_resources_api::Names::new("members.*.agent", "agent"))
        }),
    })
}

/// The project kind: published on `config`, its path canonical, one per folder, and only at a folder that's there.
fn project_kind(s: &mut Setup, owner: &'static str) -> std::sync::Arc<StoredKind> {
    let info = KindInfo::new("project", "projects", owner, wisp_resources_api::schema_of::<ProjectSpec>())
        .order(10)
        .names(wisp_resources_api::Names::new("agent", "agent"));
    let k = std::sync::Arc::new(
        StoredKind::new(info, s.ctx())
            .published("project")
            .tidied(project::tidy_project)
            .unique_by("path", project::folder_identity)
            .checked(project::check_project),
    );
    // a project by its repo is cloned on apply (ADR-0148 §7)
    let as_kind: std::sync::Arc<dyn ResourceKind> = std::sync::Arc::new(project_repo::ProjectKind::new(k.clone(), s.ctx()));
    s.contribute(&wisp_resources_api::RESOURCE_KIND, as_kind);
    let start = k.clone();
    s.on_start(move |_| async move { start.ensure() });
    k
}
