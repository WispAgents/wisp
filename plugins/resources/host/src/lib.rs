//! wisp.resources host: one engine over every plugin's kinds (ADR-0026).
//!
//! - `resources.kinds`: what this wisp can declare (each kind's section, owner, scope and schema).
//! - `resources.plan`: the formula's resources against what is here: creates, updates (a desired spec
//!   patches the current one), and with `prune` deletes; each spec checked against its kind's schema;
//!   `${secret:NAME}` references that don't exist here; the sensitive ops in plain words.
//! - `resources.apply`: the same plan, then each op through its owner (`ResourceKind::apply`, which
//!   runs the owner's own commands). Refused while the plan has problems or missing secrets, and for a
//!   foreign formula's sensitive ops without `trust`.
//! - `resources.export`: every shared kind's resources (personal and device kinds when included).
//!
//! It never touches task history (ADR-0029): tasks aren't resources, so nothing here deletes one.

mod applied;

mod capability;

mod overlay;

use std::collections::BTreeSet;
use std::sync::Arc;

use wisp_kernel::{Call, Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_resources_api::{
    Applied, Apply, Export, ExportArgs, KindInfo, Kinds, Op, OpKind, Plan, PlanArgs, PlanCmd, RESOURCE_KIND, Read, ReadOut, Refresh,
    RefreshArgs, Rename, RenameArgs, Resource, ResourceKind, Scope, Write, WriteOut,
};
use wisp_resources_core::{formula, plan, schema};

pub use wisp_resources_api::PLUGIN_ID;

/// The kinds there are: each contributed one but those put away for now ([`KindInfo::away`]: the router while Auto is
/// off, setup audit D2), in apply order.
fn kinds(ctx: &Ctx) -> Vec<Arc<dyn ResourceKind>> {
    let mut ks = ctx.contributions(&RESOURCE_KIND);
    ks.retain(|k| k.info().away.is_none());
    ks.sort_by_key(|k| {
        let i = k.info();
        (i.order, i.kind)
    });
    ks
}

/// What a formula is read against: the kinds there are and those put away, so a section naming one is refused with its
/// reason (the formula reader refuses it, audit E6).
fn formula_kinds(ctx: &Ctx) -> Vec<KindInfo> {
    let mut ks: Vec<KindInfo> = ctx.contributions(&RESOURCE_KIND).iter().map(|k| k.info()).collect();
    ks.sort_by_key(|i| (i.order, i.kind.clone()));
    ks
}

/// Why `name` is no kind here: put away for now (its reason), else no plugin owns it.
fn no_kind(ctx: &Ctx, name: &str) -> String {
    ctx.contributions(&RESOURCE_KIND)
        .iter()
        .map(|k| k.info())
        .find(|i| i.named(name))
        .and_then(|i| i.away)
        .map_or_else(|| format!("no plugin in this wisp owns the kind `{name}`"), |why| format!("`{name}`: {why}"))
}

/// The kind a resource names (by kind or by section).
fn find<'a>(ks: &'a [Arc<dyn ResourceKind>], name: &str) -> Option<&'a Arc<dyn ResourceKind>> {
    ks.iter().find(|k| k.info().named(name))
}

fn verb(op: OpKind) -> &'static str {
    match op {
        OpKind::Create => "adds",
        OpKind::Update => "changes",
        OpKind::Delete => "removes",
    }
}

/// A secret's name, checked (`wisp_resources_api::secret_name_ok`).
fn secret_name(name: &str) -> Result<String> {
    let n = name.trim();
    if wisp_resources_api::secret_name_ok(n) {
        Ok(n.to_owned())
    } else {
        Err(WireError::invalid(format!("{n:?} isn't a secret name: letters, digits, _ and - only")))
    }
}

/// The plan's drift (wisp's copy isn't what was last applied) and the secrets it names that aren't here.
async fn drift_and_secrets(ctx: &Ctx, out: &mut Plan) -> Result<()> {
    // drift: wisp's copy isn't what was last applied (the files' change is the rest of the diff)
    let base = applied::all(ctx).await.unwrap_or_default();
    for o in out.ops.iter().filter(|o| o.op == OpKind::Update) {
        if let (Some(was), Some(now)) = (base.get(&applied::key(&o.kind, &o.name)), o.before.as_ref())
            && was != now
        {
            out.drift.push(applied::key(&o.kind, &o.name));
        }
    }
    // a secret is referenced, never copied: it must already be here
    let mut names = BTreeSet::new();
    for o in out.ops.iter().filter(|o| o.op != OpKind::Delete) {
        formula::secrets_in(&o.spec, &mut names);
    }
    if !names.is_empty() {
        let secrets = wisp_kernel::services::secrets(ctx)?;
        for n in names {
            if secrets.get(&n).await?.is_none() {
                out.missing_secrets.push(n);
            }
        }
    }
    Ok(())
}

/// The references ([`ResourceKind::refs`]) that name nothing as the apply would leave it (ADR-0110): `at` (the naming
/// resource, `tasks.fix-build`) and what it names. A kind this plan doesn't touch is read as it is; `default` is no
/// resource to name.
async fn dangling(
    ks: &[Arc<dyn ResourceKind>],
    call: &Call,
    refs: Vec<(String, wisp_resources_api::Ref)>,
    mut names_after: std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // what a reference may name besides the kind's resources (a harness for an agent field), asked once per kind
    let mut also: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (at, r) in refs {
        if !names_after.contains_key(&r.kind) {
            let Some(k) = find(ks, &r.kind) else {
                out.push(format!("{at}: {}: no {} kind here", r.field, r.kind));
                continue;
            };
            let now = k.read(call).await.map(|rs| rs.into_iter().map(|x| x.name).collect()).unwrap_or_default();
            names_after.insert(r.kind.clone(), now);
        }
        if !also.contains_key(&r.kind) {
            let more = match find(ks, &r.kind) {
                Some(k) => k.also_named(call).await,
                None => Vec::new(),
            };
            also.insert(r.kind.clone(), more);
        }
        let names: Vec<&String> = names_after.get(&r.kind).into_iter().flatten().filter(|n| n.as_str() != "default").collect();
        let more: &[String] = also.get(&r.kind).map_or(&[], Vec::as_slice);
        if (r.name != "default" && names.iter().any(|n| **n == r.name)) || more.contains(&r.name) {
            continue;
        }
        let section = find(ks, &r.kind).map_or_else(|| r.kind.clone(), |k| k.info().section);
        let mut has = if names.is_empty() {
            format!("no {section} are declared")
        } else {
            format!("{section} has {}", wisp_resources_core::overlay::some_of(&names))
        };
        if !more.is_empty() {
            has = format!("{has} (or {})", wisp_resources_core::overlay::some_of(more));
        }
        let line = format!("{at}: no {} named {}: {has}", r.kind, r.name);
        if !out.contains(&line) {
            out.push(line);
        }
    }
    out
}

/// What `info.names` finds in `r`'s spec, as references ([`wisp_resources_api::Names`], ADR-0111).
fn named_refs(info: &KindInfo, r: &Resource) -> Vec<wisp_resources_api::Ref> {
    let mut out = Vec::new();
    for n in &info.names {
        for name in wisp_resources_api::names_in(&r.spec, n) {
            out.push(wisp_resources_api::Ref { kind: n.kind.clone(), name, field: n.field.clone() });
        }
    }
    out
}

/// A loop through a field that may not loop ([`wisp_resources_api::Names::acyclic`]): `crew.Ada → crew.Bo → crew.Ada`
/// over the kind's resources as the apply leaves them. One the home already had is left alone (it isn't this plan's).
/// Linear in members and edges (a depth-first walk with colours, no recursion): review r194 #4 found the old walk over
/// every path factorial in a dense crew.
fn loops(info: &KindInfo, after: &[Resource], current: &[Resource]) -> Vec<String> {
    let mut out = Vec::new();
    for n in info.names.iter().filter(|n| n.acyclic && n.kind == info.kind) {
        let found = |rs: &[Resource]| -> Vec<String> {
            let edges: std::collections::BTreeMap<&str, Vec<String>> =
                rs.iter().map(|r| (r.name.as_str(), wisp_resources_api::names_in(&r.spec, n))).collect();
            // 0 unseen, 1 on the walk's path, 2 done
            let mut colour: std::collections::BTreeMap<&str, u8> = edges.keys().map(|k| (*k, 0)).collect();
            let mut seen: Vec<String> = Vec::new();
            for start in edges.keys() {
                if colour[start] != 0 {
                    continue;
                }
                let mut path: Vec<&str> = vec![start];
                let mut next: Vec<usize> = vec![0];
                colour.insert(start, 1);
                while let (Some(&at), Some(i)) = (path.last(), next.last_mut()) {
                    let Some(to) = edges[at].get(*i) else {
                        colour.insert(at, 2);
                        path.pop();
                        next.pop();
                        continue;
                    };
                    *i += 1;
                    let Some((&to, &c)) = colour.get_key_value(to.as_str()) else { continue };
                    match c {
                        0 => {
                            colour.insert(to, 1);
                            path.push(to);
                            next.push(0);
                        }
                        1 => {
                            // back to a member on the path: the loop is that part of it, said from its smallest name
                            let from = path.iter().position(|p| *p == to).unwrap_or(0);
                            let mut cyc: Vec<&str> = path[from..].to_vec();
                            let low = cyc.iter().enumerate().min_by_key(|(_, m)| **m).map_or(0, |(k, _)| k);
                            cyc.rotate_left(low);
                            cyc.push(cyc[0]);
                            let line = format!(
                                "{}.{}: {}: a loop: {}",
                                info.section,
                                cyc[0],
                                n.field,
                                cyc.iter().map(|c| format!("{}.{c}", info.kind)).collect::<Vec<_>>().join(" → ")
                            );
                            if !seen.contains(&line) {
                                seen.push(line);
                            }
                        }
                        _ => {}
                    }
                }
            }
            seen
        };
        let had = found(current);
        out.extend(found(after).into_iter().filter(|l| !had.contains(l)));
    }
    out
}

/// The references of resources this plan leaves as they are, to a kind it removes something of (a task naming a goal
/// that `--prune` or `destroy` takes away): the ones it makes or changes are checked where they're planned.
async fn kept_refs(ks: &[Arc<dyn ResourceKind>], call: &Call, ops: &[Op]) -> Vec<(String, wisp_resources_api::Ref)> {
    let removed: BTreeSet<&str> = ops.iter().filter(|o| o.op == OpKind::Delete).map(|o| o.kind.as_str()).collect();
    let mut out = Vec::new();
    if removed.is_empty() {
        return out;
    }
    for k in ks {
        let info = k.info();
        let mut to = k.refers_to();
        to.extend(info.names.iter().map(|n| n.kind.clone()));
        if !to.iter().any(|t| removed.contains(t.as_str())) {
            continue;
        }
        let Ok(current) = k.read(call).await else { continue };
        for r in current.iter().filter(|r| !ops.iter().any(|o| o.kind == info.kind && o.name == r.name)) {
            for x in k.refs(r).into_iter().chain(named_refs(&info, r)).filter(|x| removed.contains(x.kind.as_str())) {
                out.push((format!("{}.{}", info.section, r.name), x));
            }
        }
    }
    out
}

async fn make_plan(ctx: &Ctx, call: &Call, a: &PlanArgs) -> Result<Plan> {
    let ks = kinds(ctx);
    let mut refs: Vec<(String, wisp_resources_api::Ref)> = Vec::new();
    let mut names_after: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    let mut out = Plan::default();
    let mut infos: Vec<KindInfo> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for r in &a.resources {
        if find(&ks, &r.kind).is_none() && !seen.contains(&r.kind) {
            out.problems.push(no_kind(ctx, &r.kind));
            seen.push(r.kind.clone());
        }
    }
    for k in &ks {
        let info = k.info();
        let desired: Vec<Resource> = a
            .resources
            .iter()
            .filter(|r| info.named(&r.kind))
            .map(|r| {
                let mut spec = r.spec.clone();
                k.normalize_named(&r.name, &mut spec);
                Resource { kind: info.kind.clone(), spec, ..r.clone() }
            })
            .collect();
        let gone: Vec<&str> =
            a.destroy.iter().filter_map(|d| d.split_once('.')).filter(|(kind, _)| info.named(kind)).map(|(_, n)| n).collect();
        // a section the formula names with nothing left in it (`triggers: []`) still prunes: its last one went
        let emptied = a.prune && a.kinds.iter().any(|k| info.named(k));
        if desired.is_empty() && gone.is_empty() && !emptied {
            continue;
        }
        let current = k.read(call).await?;
        if !gone.is_empty() {
            let ops: Vec<Op> = current
                .iter()
                .filter(|c| gone.contains(&c.name.as_str()))
                .map(|c| Op {
                    op: OpKind::Delete,
                    kind: info.kind.clone(),
                    name: c.name.clone(),
                    spec: c.spec.clone(),
                    id: c.id.clone(),
                    changes: Vec::new(),
                    before: None,
                })
                .collect();
            if info.sensitive {
                for o in &ops {
                    out.sensitive.push(k.describe(o).unwrap_or_else(|| format!("{} the {} {}", verb(o.op), info.kind, o.name)));
                }
            }
            names_after
                .insert(info.kind.clone(), current.iter().filter(|c| !gone.contains(&c.name.as_str())).map(|c| c.name.clone()).collect());
            out.ops.extend(ops);
            infos.push(info);
            continue;
        }
        let ops = if let Some(ops) = k.plan(&current, &desired, a.prune) {
            out.unchanged += desired.len().saturating_sub(ops.iter().filter(|o| o.op != OpKind::Delete).count());
            ops
        } else {
            let (ops, same) = plan::plan(&current, &desired, a.prune);
            out.unchanged += same;
            ops
        };
        for o in ops.iter().filter(|o| o.op != OpKind::Delete) {
            let r = Resource::new(info.kind.clone(), o.name.clone(), o.spec.clone());
            // a map-form kind's schema is the whole map's: its own check validates each value
            if !info.map_form {
                out.problems.extend(schema::validate(&info.schema, &o.spec, &format!("{}.{}", info.section, o.name)));
            }
            out.problems.extend(k.check(&r).into_iter().map(|p| format!("{}.{}: {p}", info.section, o.name)));
            refs.extend(k.refs(&r).into_iter().chain(named_refs(&info, &r)).map(|x| (format!("{}.{}", info.section, o.name), x)));
        }
        if info.sensitive {
            for o in &ops {
                out.sensitive.push(k.describe(o).unwrap_or_else(|| format!("{} the {} {}", verb(o.op), info.kind, o.name)));
            }
        }
        // the kind's resources as the apply leaves them: what's kept, what's made or changed
        let mut after: Vec<Resource> = current.iter().filter(|c| !ops.iter().any(|o| o.name == c.name)).cloned().collect();
        after.extend(
            ops.iter().filter(|o| o.op != OpKind::Delete).map(|o| Resource::new(info.kind.clone(), o.name.clone(), o.spec.clone())),
        );
        // only what this apply brings: a problem the home already has (two projects for one folder made before
        // projects were one per folder) never blocks an apply that doesn't touch it
        let had = k.check_all(&current);
        out.problems.extend(k.check_all(&after).into_iter().filter(|p| !had.contains(p)).map(|p| format!("{}: {p}", info.section)));
        // the loop check walks the whole kind: off the async worker
        if info.names.iter().any(|n| n.acyclic) {
            let (i, a, c) = (info.clone(), after.clone(), current.clone());
            out.problems.extend(tokio::task::spawn_blocking(move || loops(&i, &a, &c)).await.unwrap_or_default());
        }
        names_after.insert(info.kind.clone(), after.iter().map(|r| r.name.clone()).collect());
        out.ops.extend(ops);
        infos.push(info);
    }
    refs.extend(kept_refs(&ks, call, &out.ops).await);
    out.problems.extend(dangling(&ks, call, refs, names_after).await);
    plan::order(&mut out.ops, &infos);
    drift_and_secrets(ctx, &mut out).await?;
    Ok(out)
}

async fn apply(ctx: &Ctx, call: &Call, a: &PlanArgs) -> Result<Applied> {
    let p = make_plan(ctx, call, a).await?;
    if !p.problems.is_empty() {
        return Err(WireError::invalid(format!("the formula has problems: {}", p.problems.join("; "))));
    }
    if !p.missing_secrets.is_empty() {
        return Err(WireError::invalid(format!("these secrets aren't set here: {} (wisp secret set NAME)", p.missing_secrets.join(", "))));
    }
    if a.foreign && !a.trust && !p.sensitive.is_empty() {
        return Err(WireError::forbidden(format!(
            "this formula comes from elsewhere and changes who may act: {}. Apply it with --trust once you've read it",
            p.sensitive.join("; ")
        )));
    }
    let ks = kinds(ctx);
    let mut done: Vec<&Op> = Vec::new();
    for op in &p.ops {
        let Some(k) = find(&ks, &op.kind) else { continue };
        if let Err(e) = apply_one(k.as_ref(), call, op).await {
            // all or nothing: what went before is undone, last first, through the same owners
            let left = rollback(&ks, call, &done).await;
            let tail = if left.is_empty() {
                format!("nothing was changed ({} undone)", done.len())
            } else {
                format!("these couldn't be undone: {}", left.join("; "))
            };
            return Err(WireError::new(e.code, format!("{} ({tail})", e.message)));
        }
        done.push(op);
    }
    let base: Vec<(String, Option<serde_json::Value>)> =
        done.iter().map(|o| (applied::key(&o.kind, &o.name), (o.op != OpKind::Delete).then(|| o.spec.clone()))).collect();
    if let Err(e) = applied::set(ctx, base).await {
        tracing::warn!("resources: the applied baseline wasn't kept: {}", e.message);
    }
    Ok(Applied { applied: done.len(), plan: p })
}

/// Undoes `done` (last first); what couldn't be undone, in words.
async fn rollback(ks: &[Arc<dyn ResourceKind>], call: &Call, done: &[&Op]) -> Vec<String> {
    let mut left = Vec::new();
    for op in done.iter().rev() {
        let Some(k) = find(ks, &op.kind) else { continue };
        let now = k.read(call).await.ok().and_then(|rs| rs.into_iter().find(|r| r.name == op.name));
        let Some(back) = plan::undo(op, now.as_ref()) else {
            if op.op == OpKind::Update {
                left.push(format!("{}/{}: no spec to put back", op.kind, op.name));
            }
            continue;
        };
        if let Err(e) = k.apply(call, &back).await {
            left.push(format!("{}/{}: {}", op.kind, op.name, e.message));
        }
    }
    left
}

async fn apply_one(k: &dyn ResourceKind, call: &Call, op: &Op) -> Result<()> {
    k.apply(call, op).await.map_err(|e| WireError::new(e.code, format!("{}/{}: {}", op.kind, op.name, e.message)))
}

/// `resources.rename`: in place through its kind; the baseline moves with it.
async fn rename(ctx: &Ctx, call: &Call, a: &RenameArgs) -> Result<serde_json::Value> {
    let ks = kinds(ctx);
    let k = find(&ks, &a.kind).ok_or_else(|| WireError::not_found(format!("no plugin owns the kind `{}`", a.kind)))?;
    let kind = k.info().kind;
    if a.from == a.to || a.to.trim().is_empty() {
        return Err(WireError::invalid("rename it to another name"));
    }
    let now = k.read(call).await?;
    if now.iter().any(|r| r.name == a.to) {
        return Err(WireError::conflict(format!("wisp has a {kind} named {} already", a.to)));
    }
    if !now.iter().any(|r| r.name == a.from) {
        return Err(WireError::not_found(format!("wisp has no {kind} named {}", a.from)));
    }
    k.rename(call, &a.from, &a.to).await?;
    let mut base = applied::all(ctx).await.unwrap_or_default();
    let was = base.remove(&applied::key(&kind, &a.from));
    applied::set(ctx, vec![(applied::key(&kind, &a.from), None), (applied::key(&kind, &a.to), was)]).await?;
    Ok(serde_json::json!({ "kind": kind, "from": a.from, "to": a.to }))
}

/// `resources.refresh`: what wisp has now is the baseline (drift accepted). → what was refreshed.
async fn refresh(ctx: &Ctx, call: &Call, a: &RefreshArgs) -> Result<Vec<String>> {
    let ks = kinds(ctx);
    let mut items = Vec::new();
    for r in &a.resources {
        let Some((kind, name)) = r.split_once('/') else { continue };
        let Some(k) = find(&ks, kind) else { continue };
        if let Some(now) = k.read(call).await?.into_iter().find(|x| x.name == name) {
            items.push((applied::key(&k.info().kind, name), Some(now.spec)));
        }
    }
    let done: Vec<String> = items.iter().map(|(k, _)| k.clone()).collect();
    applied::set(ctx, items).await?;
    Ok(done)
}

async fn export(ctx: &Ctx, call: &Call, a: &ExportArgs) -> Result<Vec<Resource>> {
    let mut out = Vec::new();
    for k in kinds(ctx) {
        let i = k.info();
        let named = a.kinds.is_empty() || a.kinds.iter().any(|x| i.named(x));
        let included = i.scope == Scope::Shared || a.include.iter().any(|x| i.named(x)) || (!a.kinds.is_empty() && named);
        if !(named && included) {
            continue;
        }
        out.extend(k.read(call).await?.into_iter().map(|r| Resource { id: None, ..r }));
    }
    Ok(out)
}

/// `yaml` (or anything else), `json`, `toml`.
fn format_named(f: &str) -> wisp_resources_core::formula::Format {
    match f {
        "json" => wisp_resources_core::formula::Format::Json,
        "toml" => wisp_resources_core::formula::Format::Toml,
        _ => wisp_resources_core::formula::Format::Yaml,
    }
}

#[derive(Debug, Default)]
pub struct ResourcesPlugin;

impl Plugin for ResourcesPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        crate::capability::register(s);
        // `{ "away": true }`: the put-away kinds too, each with its `away` reason, for a formula reader to refuse their
        // section with it (audit E6); no list a person sees asks for them
        s.command::<Kinds, _, _>(|ctx, _, a| {
            let list: Vec<KindInfo> = if a.get("away") == Some(&serde_json::Value::Bool(true)) {
                formula_kinds(&ctx)
            } else {
                kinds(&ctx).iter().map(|k| k.info()).collect()
            };
            async move { Ok(list) }
        });
        s.command::<PlanCmd, _, _>(|ctx, call, a| async move { make_plan(&ctx, &call, &a).await });
        s.command::<Apply, _, _>(|ctx, call, a| async move { apply(&ctx, &call, &a).await });
        s.command::<Rename, _, _>(|ctx, call, a| async move { rename(&ctx, &call, &a).await });
        s.command::<Refresh, _, _>(|ctx, call, a| async move { refresh(&ctx, &call, &a).await });
        s.command::<Export, _, _>(|ctx, call, a| async move { export(&ctx, &call, &a).await });
        // the app's Import and Share setup (ADR-0034): pasted text in, the setup out as text
        s.command::<wisp_resources_api::Overlay, _, _>(|ctx, call, a| async move { overlay::overlay(&ctx, &call, kinds(&ctx), a).await });
        s.command::<Read, _, _>(|ctx, _call, a| {
            let ks = formula_kinds(&ctx);
            async move {
                let loaded = wisp_resources_core::formula::read_text(&a.text, format_named(&a.format), &ks).map_err(WireError::invalid)?;
                Ok(ReadOut { resources: loaded.resources, secrets: loaded.secrets })
            }
        });
        // a formula's `${secret:NAME}`: set here (never in the store), so `apply` finds it
        s.command::<wisp_resources_api::SecretSet, _, _>(|ctx, _call, a| async move {
            let name = secret_name(&a.name)?;
            if a.value.is_empty() {
                return Err(WireError::invalid(format!("{name}: an empty secret (remove it with wisp secret rm {name})")));
            }
            wisp_kernel::services::secrets(&ctx)?.set(&name, &a.value).await?;
            Ok(wisp_resources_api::SecretOut { name, set: true })
        });
        s.command::<wisp_resources_api::SecretRemove, _, _>(|ctx, _call, a| async move {
            let name = secret_name(&a.name)?;
            wisp_kernel::services::secrets(&ctx)?.delete(&name).await?;
            Ok(wisp_resources_api::SecretOut { name, set: false })
        });
        s.command::<Write, _, _>(|ctx, call, a| async move {
            let ks: Vec<KindInfo> = kinds(&ctx).iter().map(|k| k.info()).collect();
            let list = export(&ctx, &call, &ExportArgs { kinds: a.kinds.clone(), include: Vec::new() }).await?;
            Ok(WriteOut {
                text: wisp_resources_core::export::write(&list, &ks, format_named(&a.format)),
                count: u32::try_from(list.len()).unwrap_or(u32::MAX),
            })
        });
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod loop_tests {
    use super::*;
    use serde_json::json;

    /// Review r194 #4: the loop check walked every path (factorial in a dense crew); it's linear now. 300 members, each
    /// delegating to every later one (44 850 edges, no loop), then one edge back: answered in well under a second.
    #[test]
    fn a_large_crew_graph_is_checked_for_loops_in_linear_time() {
        let info = KindInfo::new("crew", "crew", "x", json!({})).names(wisp_resources_api::Names::new("team[].crew", "crew").acyclic());
        let n = 300;
        let mk = move |back: bool| -> Vec<Resource> {
            (0..n)
                .map(|i| {
                    let mut team: Vec<serde_json::Value> = ((i + 1)..n).map(|j| json!({ "crew": format!("m{j:03}") })).collect();
                    if back && i == n - 1 {
                        team.push(json!({ "crew": "m000" }));
                    }
                    Resource::new("crew", format!("m{i:03}"), json!({ "team": team }))
                })
                .collect()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let info2 = info.clone();
        std::thread::spawn(move || {
            let none = loops(&info2, &mk(false), &[]);
            let one = loops(&info2, &mk(true), &[]);
            let _ = tx.send((none, one));
        });
        let (none, one) = rx.recv_timeout(std::time::Duration::from_secs(3)).expect("the loop check took over 3 s");
        assert!(none.is_empty(), "{none:?}");
        assert_eq!(one.len(), 1, "one loop, said once: {one:?}");
        assert!(one[0].starts_with("crew.m000: team[].crew: a loop: crew.m000 → crew.m001 → "), "{}", one[0]);
        assert!(one[0].ends_with("crew.m299 → crew.m000"), "{}", one[0]);
    }
}
