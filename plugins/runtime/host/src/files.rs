//! A turn's attached files, as its harness gets them (v1 runner `uploads.resolve`). A turn keeps an upload's id,
//! name, mime and size only (what the page sends back from `POST /uploads`); the file on disk is wisp.stuff's, so
//! the run asks it by id at start. A client never names the path itself: only an id wisp.stuff knows gets one, and
//! an id it doesn't know keeps what it came with (a channel's file, already on disk).

use wisp_runtime_api::model::Attachment;

use crate::runtime::Runtime;

impl Runtime {
    /// `files` with each known upload's readable path, in their order. A failed lookup leaves them as they were
    /// (the harness then lists none of them, as before), with a warning: it never stops the run.
    pub(crate) async fn files_for_run(&self, files: &[Attachment]) -> Vec<Attachment> {
        let ids: Vec<String> = files.iter().filter(|a| !a.id.is_empty()).map(|a| a.id.clone()).collect();
        if ids.is_empty() {
            return files.to_vec();
        }
        let found = match crate::calls::run::<wisp_stuff_api::commands::Uploads>(&*self.host, wisp_stuff_api::commands::Ids { ids }).await {
            Ok(u) => u,
            Err(err) => {
                tracing::warn!(error = %err.message, "runtime: stuff.uploads for a turn's attachments");
                return files.to_vec();
            }
        };
        files
            .iter()
            .map(|a| match found.iter().find(|u| u.id == a.id) {
                Some(u) => Attachment { path: u.path.clone(), ..a.clone() },
                None => a.clone(),
            })
            .collect()
    }
}
