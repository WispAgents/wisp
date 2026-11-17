//! `plugins.styles` (ADR-0007, ADR-0048): the `contributes.partStyles` of every plugin that is on and trusted (a
//! built-in, or one of yours you trusted), for the shell to write into `@layer plugins`. An untrusted or switched-off
//! plugin gives none. Each map was checked against the allow-list when its manifest loaded
//! (`wisp_kernel::part_styles`); the shell compiles with the same list, so nothing else reaches the page.
//! When the answer may have changed (a plugin switched, trusted, installed or removed), every client gets a
//! `plugins.changed` notice and asks again.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use wisp_kernel::{Ctx, Manifest, Result};

/// The notice every client gets when what `plugins.styles` (or `plugins.ui`) answers may have changed.
pub const CHANGED: &str = "plugins.changed";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStyles {
    pub plugin: String,
    pub part_styles: Value,
}

/// A built-in (no trust field, or "trusted") or a plugin of yours you trusted (install.rs sets it from the mark).
pub fn trusted(m: &Manifest) -> bool {
    m.trust.as_deref().is_none_or(|t| t == "trusted")
}

/// The styles of the plugins in `on` (by id) that are trusted and have any, in `manifests` order.
pub fn pick(manifests: &[Manifest], on: &[String]) -> Vec<PluginStyles> {
    manifests
        .iter()
        .filter(|m| on.contains(&m.id) && trusted(m))
        .filter_map(|m| {
            let s = m.contributes.get("partStyles")?;
            (s.as_object().is_some_and(|o| !o.is_empty()) && wisp_kernel::part_styles::check(s).is_empty())
                .then(|| PluginStyles { plugin: m.id.clone(), part_styles: s.clone() })
        })
        .collect()
}

pub fn list(ctx: &Ctx) -> Result<Vec<PluginStyles>> {
    let on: Vec<String> = ctx.plugins()?.into_iter().filter(|p| p.state != "off").map(|p| p.id).collect();
    Ok(pick(&ctx.manifests(), &on))
}

/// Tells every client to ask again (`plugins.changed`); a wisp without clients has no notify service.
pub fn changed(ctx: &Ctx, id: &str) {
    if let Ok(n) = wisp_kernel::services::notify(ctx) {
        n.notice(CHANGED, serde_json::json!({ "id": id }), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(id: &str, trust: Option<&str>, styles: &Value) -> Manifest {
        let mut m = Manifest::embedded(
            &json!({ "wispPlugin": 1, "id": id, "name": id, "version": "1.0.0", "api": "^1", "contributes": { "partStyles": styles } })
                .to_string(),
        );
        m.trust = trust.map(str::to_owned);
        m
    }

    #[test]
    fn only_trusted_plugins_that_are_on_give_styles() {
        let s = json!({ "composer.send": { "background": "hotpink" } });
        let ms = [
            manifest("user.mine", Some("trusted"), &s),
            manifest("user.theirs", Some("untrusted"), &s),
            manifest("user.off", Some("trusted"), &s),
            manifest("wisp.theme", None, &s),
            manifest("user.empty", Some("trusted"), &json!({})),
        ];
        let on = ["user.mine", "user.theirs", "wisp.theme", "user.empty"].map(str::to_owned);
        let got: Vec<String> = pick(&ms, &on).into_iter().map(|p| p.plugin).collect();
        assert_eq!(got, ["user.mine", "wisp.theme"]);
    }

    #[test]
    fn styles_the_allow_list_refuses_never_leave() {
        let ms = [manifest("user.bad", Some("trusted"), &json!({ "composer.send": { "background": "red; } body { display: none" } }))];
        assert_eq!(pick(&ms, &["user.bad".to_owned()]), Vec::<PluginStyles>::new());
    }
}
