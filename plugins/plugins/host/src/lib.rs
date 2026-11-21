//! wisp-plugins: the plugin manager. `plugins.list` (what the kernel has, and each one's state) and
//! `plugins.setEnabled` (the kernel switches it on or off, and everything it registered with it); the
//! `plugin` resource kind (ADR-0028) does the same from a formula, on apply and at start.

mod agent;
mod install;
mod kinds;
pub mod remote;
pub mod styles;
mod swap;

pub use install::{UserPlugin, at_start as user_plugins, read as read_plugin};

use wisp_kernel::{Ctx, Manifest, Plugin, PluginStatus, Result, Setup, WireError};
use wisp_plugins_api::{List, PluginRow, SetEnabled};

/// The plugins that stay on: this one (the switches themselves) and `wisp.settings`, where every setting lives. Off,
/// All settings read "No setting matches." and every switch failed with a toast, saying nothing of why (audit F setup
/// F7); so it can't be switched off, as this one can't.
pub const STAYS_ON: [&str; 2] = ["wisp.plugins", "wisp.settings"];

/// Why `id` can't be switched off (None: it can, or it's being switched on).
pub fn stays_on(id: &str, enabled: bool) -> Option<String> {
    (!enabled && STAYS_ON.contains(&id)).then(|| format!("{id} can't be switched off: wisp needs it to keep its settings and plugins"))
}

/// `plugins.install` (ADR-0046): a folder or a git URL into `~/.wisp/plugins`, added off.
#[derive(Debug)]
pub struct Install;
impl wisp_kernel::Command for Install {
    const ID: &'static str = "plugins.install";
    const PERMISSION: Option<&'static str> = Some(wisp_plugins_api::perm::WRITE);
    type Args = install::InstallArgs;
    type Out = install::Installed;
    // typed (audit D6): `wisp plugins install <source>`, and the tool's one field
    fn args_ts() -> Option<String> {
        Some("{ \n/**\n * a folder on this computer, or a git URL\n */\nsource: string, }".to_owned())
    }
}

/// `plugins.uninstall` (ADR-0046): a `user.` plugin off, out, and its folder gone.
#[derive(Debug)]
pub struct Uninstall;
impl wisp_kernel::Command for Uninstall {
    const ID: &'static str = "plugins.uninstall";
    const PERMISSION: Option<&'static str> = Some(wisp_plugins_api::perm::WRITE);
    type Args = install::UninstallArgs;
    type Out = serde_json::Value;
    fn args_ts() -> Option<String> {
        Some("{ \n/**\n * the plugin (`user.…`)\n */\nid: string, }".to_owned())
    }
}

/// `plugins.trust` (ADR-0048): a plugin of yours trusted, or back to untrusted.
#[derive(Debug)]
pub struct Trust;
impl wisp_kernel::Command for Trust {
    const ID: &'static str = "plugins.trust";
    const PERMISSION: Option<&'static str> = Some(wisp_plugins_api::perm::WRITE);
    type Args = install::TrustArgs;
    type Out = serde_json::Value;
    fn args_ts() -> Option<String> {
        Some("{ \n/**\n * the plugin (`user.…`)\n */\nid: string, \n/**\n * trust it (its own process with your environment), or take the trust back\n */\ntrusted: boolean, }".to_owned())
    }
}

/// `plugins.folder`: where this wisp keeps your plugins (`<WISP_HOME>/plugins`), for Settings → Plugins' foot (audit
/// C9: it said `~/.wisp/plugins` whatever `WISP_HOME` was).
#[derive(Debug)]
pub struct Folder;
impl wisp_kernel::Command for Folder {
    const ID: &'static str = "plugins.folder";
    const PERMISSION: Option<&'static str> = Some(wisp_plugins_api::perm::READ);
    const READ_ONLY: bool = true;
    type Args = serde_json::Value;
    fn args_ts() -> Option<String> {
        wisp_plugins_api::any_args_ts()
    }
    type Out = FolderOut;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FolderOut {
    pub path: String,
}

/// `plugins.ui` (ADR-0048): untrusted plugins' UI modules, for the shell's remote view host.
#[derive(Debug)]
pub struct Ui;
impl wisp_kernel::Command for Ui {
    const ID: &'static str = "plugins.ui";
    const PERMISSION: Option<&'static str> = Some(wisp_plugins_api::perm::READ);
    const READ_ONLY: bool = true;
    type Args = serde_json::Value;
    fn args_ts() -> Option<String> {
        wisp_plugins_api::any_args_ts()
    }
    type Out = Vec<remote::RemoteUi>;
}

/// `plugins.styles` (ADR-0007): the part styles of every plugin that is on and trusted, for `@layer plugins`.
#[derive(Debug)]
pub struct Styles;
impl wisp_kernel::Command for Styles {
    const ID: &'static str = "plugins.styles";
    const PERMISSION: Option<&'static str> = Some(wisp_plugins_api::perm::READ);
    const READ_ONLY: bool = true;
    type Args = serde_json::Value;
    fn args_ts() -> Option<String> {
        wisp_plugins_api::any_args_ts()
    }
    type Out = Vec<styles::PluginStyles>;
}

#[derive(Debug, Default)]
pub struct PluginsPlugin;

fn row(p: PluginStatus) -> PluginRow {
    PluginRow {
        enabled: p.state != "off",
        id: p.id,
        version: p.version,
        state: p.state,
        builtin: p.builtin,
        reason: p.reason,
        ..PluginRow::default()
    }
}

/// The list's rows, a plugin of yours with its trust, what it asks for and its name (its folder's, audit A F19).
async fn rows(ctx: &Ctx) -> Result<Vec<PluginRow>> {
    let mut out = Vec::new();
    for p in ctx.plugins()? {
        let mut r = row(p);
        if r.id.starts_with("user.")
            && let Some(a) = install::about(ctx, &r.id).await
        {
            r.trusted = Some(a.trusted);
            r.permissions = a.permissions;
            r.name = a.name;
        }
        out.push(r);
    }
    Ok(out)
}

pub(crate) fn find(ctx: &Ctx, id: &str) -> Result<PluginRow> {
    ctx.plugins()?.into_iter().find(|p| p.id == id).map(row).ok_or_else(|| WireError::not_found(format!("no plugin {id}")))
}

impl Plugin for PluginsPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<List, _, _>(|ctx, _call, _args| async move { rows(&ctx).await });
        s.command::<Folder, _, _>(|ctx, _call, _args| async move {
            Ok(FolderOut { path: ctx.service::<wisp_kernel::services::Paths>()?.join("plugins").display().to_string() })
        });
        let kind = crate::kinds::register(s);
        let kept = kind.clone();
        s.command::<SetEnabled, _, _>(move |ctx, call, a| {
            let kind = kind.clone();
            async move {
                if let Some(why) = stays_on(&a.id, a.enabled) {
                    return Err(WireError::invalid(why));
                }
                // a user's plugin known by its folder only (`wisp plugin new`): loaded, so it's listed, on or off (setup
                // audit F18: one you were writing showed nowhere until you switched it on)
                install::load_from_its_folder(&ctx, &a.id).await?;
                ctx.set_plugin_enabled(&a.id, a.enabled).await?;
                // kept so it comes back that way; a store that can't keep it doesn't undo the switch
                if let Err(e) = crate::kinds::remember(&kind, &call, &a.id, a.enabled).await {
                    tracing::warn!(plugin = %a.id, "its switch isn't kept: {}", e.message);
                }
                styles::changed(&ctx, &a.id);
                find(&ctx, &a.id)
            }
        });
        // ADR-0046: a user's plugin from a folder or a git URL, and taking one out
        // each tells the clients when what they show (styles, untrusted UIs) may have changed: plugins.changed
        s.command::<Install, _, _>(|ctx, _call, a| async move { install::install(&ctx, &a).await.inspect(|_| styles::changed(&ctx, "")) });
        s.command::<Uninstall, _, _>(move |ctx, call, a| {
            let kept = kept.clone();
            async move {
                let out = install::uninstall(&ctx, &a).await?;
                // taken out, so its kept switch goes too: never in `wisp export` or a formula, enabled (setup audit F20)
                if let Err(e) = crate::kinds::forget(&kept, &call, &a.id).await {
                    tracing::warn!(plugin = %a.id, "its kept switch stays: {}", e.message);
                }
                styles::changed(&ctx, &a.id);
                Ok(out)
            }
        });
        s.command::<Trust, _, _>(|ctx, _call, a| async move { install::trust(&ctx, &a).await.inspect(|_| styles::changed(&ctx, &a.id)) });
        s.command::<Ui, _, _>(|ctx, _call, _a| async move { remote::list(&ctx).await });
        s.command::<Styles, _, _>(|ctx, _call, _a| async move { styles::list(&ctx) });
        // ADR-0048, ADR-0040: what an agent writing a plugin reads and runs (wisp_plugins, wisp_plugin_*)
        agent::register(s);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit F setup F7: with wisp.settings off, All settings said "No setting matches." and switches failed with a
    /// toast. It stays on, as wisp.plugins does; every other built-in still switches off and on.
    #[test]
    fn the_settings_plugin_stays_on_audit_f_setup_f7() {
        assert!(stays_on("wisp.settings", false).is_some_and(|w| w.contains("can't be switched off")));
        assert!(stays_on("wisp.plugins", false).is_some());
        assert_eq!(stays_on("wisp.settings", true), None, "switching it on is fine");
        assert_eq!(stays_on("wisp.learn", false), None);
    }

    #[test]
    fn manifest_is_valid() {
        let m = PluginsPlugin.manifest();
        assert_eq!(m.id, "wisp.plugins");
        assert!(m.validate().is_ok());
    }
}
