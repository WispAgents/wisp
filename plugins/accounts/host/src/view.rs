//! What Settings → Agents reads, as flat rows on `plugin:wisp.accounts:harnesses` (the UI's types.ts, field
//! for field): `accounts.harness` per harness (`runtime.agents` and each harness's sign-in kind),
//! `accounts.account` per extra account, `accounts.login` per sign-in under way, `accounts.key` per
//! provider key (never the key), `accounts.tool` for git and gh, `accounts.place` (`this`). Written whole
//! after every change (`publish`); a row that's gone is deleted.

use std::collections::HashSet;

use serde_json::{Value, json};
use wisp_accounts_api::TOPIC;
use wisp_kernel::Ctx;
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};

use crate::{State, lock, now};

const MIGRATIONS: &[Migration] = &[
    Migration::sql(
    1,
    "CREATE TABLE accounts_v_harness (id TEXT PRIMARY KEY, name TEXT NOT NULL, ord INTEGER NOT NULL, available INTEGER NOT NULL, version TEXT, models TEXT,
       signedIn INTEGER, error TEXT, hasKey INTEGER, setup TEXT, signIn TEXT NOT NULL, installable INTEGER, installing INTEGER, brand TEXT, provider TEXT,
       changed TEXT, pinnedAt INTEGER) STRICT;
     CREATE TABLE accounts_v_account (id TEXT PRIMARY KEY, agent TEXT NOT NULL, ord INTEGER NOT NULL, label TEXT NOT NULL, email TEXT NOT NULL, plan TEXT NOT NULL,
       \"default\" INTEGER NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL, until INTEGER, why TEXT NOT NULL) STRICT;
     CREATE TABLE accounts_v_login (id TEXT PRIMARY KEY, agent TEXT NOT NULL, account TEXT NOT NULL, state TEXT NOT NULL, url TEXT, code TEXT, needsInput INTEGER,
       checking INTEGER, error TEXT, signedIn INTEGER) STRICT;
     CREATE TABLE accounts_v_key (id TEXT PRIMARY KEY, agent TEXT NOT NULL, agentName TEXT NOT NULL, provider TEXT NOT NULL, name TEXT NOT NULL, url TEXT NOT NULL,
       env TEXT NOT NULL, \"set\" INTEGER NOT NULL, hint TEXT NOT NULL, savedAt INTEGER, test TEXT) STRICT;
     CREATE TABLE accounts_v_tool (id TEXT PRIMARY KEY, name TEXT NOT NULL, installed INTEGER NOT NULL, setup TEXT) STRICT;
     CREATE TABLE accounts_v_place (id TEXT PRIMARY KEY, grid INTEGER NOT NULL, ask INTEGER NOT NULL) STRICT;",
    ),
    // TRY-17: a newer version than the one installed, when there is one
    Migration::sql(2, "ALTER TABLE accounts_v_harness ADD COLUMN latest TEXT;"),
    // QA r1 #23: whether gh is signed in (the GitHub row said nothing)
    Migration::sql(3, "ALTER TABLE accounts_v_tool ADD COLUMN signedIn INTEGER;"),
    // Settings → Agents → Binary: the program wisp pinned (as found, its real path, its version)
    Migration::sql(4, "ALTER TABLE accounts_v_harness ADD COLUMN pin TEXT;"),
];

fn topics(_: &Row) -> Vec<String> {
    vec![TOPIC.into()]
}

pub const HARNESS: EntityDef = EntityDef {
    kind: "accounts.harness",
    table: "accounts_v_harness",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("name", Col::Text),
        ("ord", Col::Int),
        ("available", Col::Bool),
        ("version", Col::Text),
        ("models", Col::Text),
        ("signedIn", Col::Bool),
        ("error", Col::Text),
        ("hasKey", Col::Bool),
        ("setup", Col::Text),
        ("signIn", Col::Text),
        ("installable", Col::Bool),
        ("installing", Col::Bool),
        ("brand", Col::Text),
        ("provider", Col::Text),
        ("changed", Col::Text),
        ("pinnedAt", Col::Int),
        ("latest", Col::Text),
        ("pin", Col::Text),
    ],
    json_columns: &["models", "setup", "changed", "pin"],
    parent: None,
    seq: None,
    topics,
};
pub const ACCOUNT: EntityDef = EntityDef {
    kind: "accounts.account",
    table: "accounts_v_account",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("agent", Col::Text),
        ("ord", Col::Int),
        ("label", Col::Text),
        ("email", Col::Text),
        ("plan", Col::Text),
        ("default", Col::Bool),
        ("kind", Col::Text),
        ("state", Col::Text),
        ("until", Col::Int),
        ("why", Col::Text),
    ],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};
pub const LOGIN: EntityDef = EntityDef {
    kind: "accounts.login",
    table: "accounts_v_login",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("agent", Col::Text),
        ("account", Col::Text),
        ("state", Col::Text),
        ("url", Col::Text),
        ("code", Col::Text),
        ("needsInput", Col::Bool),
        ("checking", Col::Bool),
        ("error", Col::Text),
        ("signedIn", Col::Bool),
    ],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};
pub const KEY: EntityDef = EntityDef {
    kind: "accounts.key",
    table: "accounts_v_key",
    key: "id",
    columns: &[
        ("id", Col::Text),
        ("agent", Col::Text),
        ("agentName", Col::Text),
        ("provider", Col::Text),
        ("name", Col::Text),
        ("url", Col::Text),
        ("env", Col::Text),
        ("set", Col::Bool),
        ("hint", Col::Text),
        ("savedAt", Col::Int),
        ("test", Col::Text),
    ],
    json_columns: &["test"],
    parent: None,
    seq: None,
    topics,
};
pub const TOOL: EntityDef = EntityDef {
    kind: "accounts.tool",
    table: "accounts_v_tool",
    key: "id",
    columns: &[("id", Col::Text), ("name", Col::Text), ("installed", Col::Bool), ("setup", Col::Text), ("signedIn", Col::Bool)],
    json_columns: &["setup"],
    parent: None,
    seq: None,
    topics,
};
pub const PLACE: EntityDef = EntityDef {
    kind: "accounts.place",
    table: "accounts_v_place",
    key: "id",
    columns: &[("id", Col::Text), ("grid", Col::Bool), ("ask", Col::Bool)],
    json_columns: &[],
    parent: None,
    seq: None,
    topics,
};

pub const ALL: [EntityDef; 6] = [HARNESS, ACCOUNT, LOGIN, KEY, TOOL, PLACE];

pub fn install(store: &Store) -> wisp_store::Result<()> {
    store.migrate("accounts_view", MIGRATIONS)?;
    for d in ALL {
        store.define_entity(d)?;
    }
    Ok(())
}

/// A tool's row (git, gh): installed, how to get it, and for gh whether it's signed in (None: its answer unknown).
fn tool_row(id: &str, name: &str, t: &wisp_git_api::ToolState) -> Row {
    row_of(&TOOL, &json!({ "id": id, "name": name, "installed": t.installed, "setup": t.setup, "signedIn": t.signed_in }))
}

/// A JSON object as a row of `def`: its own columns only, the rest null.
pub fn row_of(def: &EntityDef, v: &Value) -> Row {
    let mut r = Row::new();
    for (c, _) in def.columns {
        r.insert((*c).to_owned(), v.get(*c).cloned().unwrap_or(Value::Null));
    }
    r
}

/// How a harness signs in: a key pasted into wisp, its CLI's own login, or neither here.
pub fn sign_in_kind(has_keys: bool, has_login: bool) -> &'static str {
    if has_keys {
        "key"
    } else if has_login {
        "cli"
    } else {
        "none"
    }
}

/// Every row, by kind, as they stand now.
pub async fn rows(ctx: &Ctx, st: &State, fresh_tools: bool) -> Option<Vec<(&'static EntityDef, Vec<Row>)>> {
    // runtime.agents refused (starting, busy): no rows, rather than none (which would delete every row the pane shows)
    let agents = match crate::calls::agents(ctx, None).await {
        Ok(a) => a,
        Err(err) => {
            tracing::warn!(error = %err.message, "accounts: runtime.agents; the rows stay as they were");
            return None;
        }
    };
    let mut harnesses = Vec::new();
    let mut keys = Vec::new();
    let mut accounts = Vec::new();
    let installing = lock(&st.installing).clone();
    for (ord, a) in agents.iter().enumerate() {
        let id = a.id.clone();
        let h = crate::harness(ctx, &id);
        let has_keys = h.as_ref().is_some_and(|h| h.keys().is_some());
        let has_login = h.as_ref().is_some_and(|h| h.login().is_some());
        let multi = h.as_ref().and_then(|h| h.accounts().map(|s| s.label.clone()));
        let w = lock(&st.watch).get(&id).cloned().unwrap_or_default();
        let wv = wisp_accounts_core::watch::view(&w);
        let mut has_key = Value::Null;
        if let Some(k) = h.as_ref().and_then(|h| h.keys().cloned())
            && let Ok(kv) = crate::keys::view(&id, &k).await
        {
            has_key = json!(kv.providers.iter().any(|p| p.set) || !kv.env.is_empty());
            for p in kv.providers {
                keys.push(row_of(&KEY, &json!({ "id": format!("{id}/{}", p.id), "agent": id, "agentName": a.name, "provider": p.id, "name": p.name, "url": kv.url, "env": kv.env, "set": p.set, "hint": "", "savedAt": null })));
            }
        }
        let latest = lock(&st.latest)
            .get(&id)
            .filter(|l| a.available && wisp_accounts_core::update::newer(a.version.as_deref().unwrap_or(""), l))
            .cloned();
        let models: Vec<Value> = a.models.iter().map(|m| json!({ "id": m.id, "label": m.label })).collect();
        harnesses.push(row_of(
            &HARNESS,
            &json!({
                "id": id, "name": a.name, "ord": ord, "available": a.available,
                "version": a.version, "models": models,
                "signedIn": if a.signed_in == Some(false) { json!(false) } else { Value::Null },
                "error": a.error, "hasKey": has_key, "setup": a.setup, "signIn": sign_in_kind(has_keys, has_login),
                "installable": h.as_ref().is_some_and(|h| h.npm_package().is_some()), "installing": installing.contains(&id),
                "brand": multi, "provider": null,
                "changed": serde_json::to_value(&wv.changed).unwrap_or(Value::Null), "pinnedAt": wv.pinned_at,
                "latest": latest,
                "pin": w.pin.as_ref().map(|p| json!({ "path": p.path, "real": p.real, "version": p.version })),
            }),
        ));
        let list = lock(&st.accounts).get(&id).cloned().unwrap_or_default();
        for (i, v) in wisp_accounts_core::accounts::view(&list, now(ctx)).into_iter().enumerate() {
            let mut x = serde_json::to_value(&v).unwrap_or(Value::Null);
            x["id"] = json!(format!("{id}/{}", v.id));
            x["agent"] = json!(id);
            x["ord"] = json!(i);
            accounts.push(row_of(&ACCOUNT, &x));
        }
    }
    // git and gh: asked again when asked to (after a command), else as last asked
    let cached = lock(&st.tools).clone();
    let tools = match cached {
        Some(t) if !fresh_tools => Some(t),
        _ => crate::calls::tools(ctx).await.inspect(|t| *lock(&st.tools) = Some(t.clone())),
    };
    let flows = crate::signin::all(st);
    // gh's own answer, freshly asked, is the GitHub row's Signed in (signed in or out in a terminal shows here too),
    // unless a sign-in from here is under way; a cached one never overrides what a sign-in from here just ended with
    if fresh_tools
        && let Some(on) = tools.as_ref().and_then(|t| t.gh.signed_in)
        && !flows.iter().any(|(f, v)| f == crate::gh::ID && v.state.live())
    {
        lock(&st.statuses).insert(crate::gh::ID.to_owned(), Some(on));
    }
    let statuses = lock(&st.statuses).clone();
    let mut logins: Vec<Row> = flows
        .iter()
        .map(|(lid, v)| {
            let mut x = serde_json::to_value(v).unwrap_or(Value::Null);
            x["id"] = json!(lid);
            // signed in or not as last known (a sign-in that ended done is): Signed in · Sign out on its row
            if let Some(Some(on)) = statuses.get(lid) {
                x["signedIn"] = json!(on);
            }
            row_of(&LOGIN, &x)
        })
        .collect();
    // the CLIs' own logins as last asked: Signed in / Sign out on their rows
    for (lid, on) in &statuses {
        if on.is_some() && !flows.iter().any(|(f, _)| f == lid) {
            let (agent, account) = lid.split_once('@').unwrap_or((lid.as_str(), ""));
            logins.push(row_of(&LOGIN, &json!({ "id": lid, "agent": agent, "account": account, "state": "idle", "signedIn": on })));
        }
    }
    let tool_rows: Vec<Row> = tools
        .map(|t| [("git", "git", t.git), ("gh", "GitHub CLI", t.gh)].into_iter().map(|(id, name, t)| tool_row(id, name, &t)).collect())
        .unwrap_or_default();
    let grid = std::env::var("WISP_GRID").is_ok_and(|v| !v.is_empty());
    Some(vec![
        (&HARNESS, harnesses),
        (&ACCOUNT, accounts),
        (&LOGIN, logins),
        (&KEY, keys),
        (&TOOL, tool_rows),
        (&PLACE, vec![row_of(&PLACE, &json!({ "id": "this", "grid": grid, "ask": st.ask() }))]),
    ])
}

/// Writes every row; a row no longer there is deleted. git's and gh's state is asked again.
pub async fn publish(ctx: &Ctx, st: &State) {
    publish_with(ctx, st, true).await;
}

/// `publish`, one at a time, the snapshot taken inside (an older one never lands last); `fresh_tools`: ask git and gh
/// again.
async fn publish_with(ctx: &Ctx, st: &State, fresh_tools: bool) {
    let _one = st.publishing.lock().await;
    let Some(all) = rows(ctx, st, fresh_tools).await else { return };
    let r = st
        .store
        .write(move |w| {
            for (def, want) in all {
                let keep: HashSet<String> = want.iter().filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_owned)).collect();
                for old in w.query(def.kind, &Query::default())? {
                    if let Some(id) = old.get("id").and_then(Value::as_str).filter(|id| !keep.contains(*id)) {
                        w.del(def.kind, id.to_owned())?;
                    }
                }
                for r in want {
                    w.upsert(def.kind, r)?;
                }
            }
            Ok(())
        })
        .await;
    if let Err(e) = r {
        tracing::warn!(error = %e, "accounts: writing Settings → Agents' rows");
    }
}

/// Asks each CLI that signs in by itself whether it's signed in (its status command), for the rows. All at once: one
/// slow CLI (up to its 8 s timeout) no longer holds the others' answers, and the rows after a change, back (the
/// owner's audit: "when I add something it takes time").
pub async fn probe(ctx: &Ctx, st: &std::sync::Arc<State>) {
    let mut asks = tokio::task::JoinSet::new();
    for h in ctx.contributions(&wisp_runtime_api::HARNESS) {
        if h.login().is_none() || h.keys().is_some() {
            continue;
        }
        let (c, s) = (ctx.clone(), st.clone());
        asks.spawn(async move {
            let agent = h.info().id.clone();
            crate::signin::status(&c, &s, h.as_ref(), "").await.ok().map(|x| (agent, x.signed_in))
        });
    }
    while let Some(r) = asks.join_next().await {
        // an answer only: a timeout or an output it can't read keeps what's known (a sign-in that ended done)
        if let Ok(Some((agent, Some(on)))) = r {
            lock(&st.statuses).insert(agent, Some(on));
        }
    }
}

/// Writes the rows again whenever something changed outside a command (`State::changed`), for as long as the plugin
/// runs.
pub fn follow(ctx: &Ctx, st: &std::sync::Arc<State>) {
    let (c, s) = (ctx.clone(), st.clone());
    ctx.spawn(async move {
        loop {
            s.changed.notified().await;
            publish_with(&c, &s, false).await;
        }
    });
}

/// The CLIs asked again, then `publish`, off the caller's way (at start, after a command).
pub fn soon(ctx: &Ctx, st: &std::sync::Arc<State>) {
    let (c, s) = (ctx.clone(), st.clone());
    ctx.spawn(async move {
        publish(&c, &s).await;
        probe(&c, &s).await;
        publish(&c, &s).await;
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A harness row is the UI's fields only, named as the UI names them; the sign-in kind follows what
    /// the harness has: keys win, then its CLI's login.
    #[tokio::test]
    async fn the_agents_pane_rows_are_flat_and_named_as_the_ui_reads_them() {
        let db = Store::open(wisp_store::Location::Memory, std::sync::Arc::new(wisp_store::SystemClock)).unwrap();
        install(&db).unwrap();
        let r = row_of(
            &HARNESS,
            &json!({ "id": "opencode", "name": "opencode", "ord": 2, "available": true, "models": [{ "id": "a", "label": "A" }], "signIn": sign_in_kind(true, true), "hasKey": false, "pinnedAt": 9, "extra": "dropped" }),
        );
        let a = row_of(
            &ACCOUNT,
            &json!({ "id": "claude/work", "agent": "claude", "ord": 0, "label": "Work", "email": "", "plan": "", "default": false, "kind": "work", "state": "ready", "until": null, "why": "" }),
        );
        db.write(move |w| w.upsert(HARNESS.kind, r).and_then(|_| w.upsert(ACCOUNT.kind, a)).map(|_| ())).await.unwrap();
        let h = db.read(|x| x.get("accounts.harness", "opencode")).unwrap().unwrap();
        assert_eq!(h["signIn"], "key");
        assert_eq!(h["hasKey"], false);
        assert_eq!(h["pinnedAt"], 9);
        assert_eq!(h["models"][0]["label"], "A");
        assert!(h.get("extra").is_none());
        assert_eq!(db.read(|x| x.get("accounts.account", "claude/work")).unwrap().unwrap()["default"], false);
        assert_eq!(sign_in_kind(false, true), "cli");
        assert_eq!(sign_in_kind(false, false), "none");
        assert_eq!((HARNESS.topics)(&h), vec![TOPIC.to_owned()]);
    }

    /// Settings audit r1: Settings → Agents → Binary had no path to show (the row carried only when it was pinned). The
    /// harness row carries the program wisp pinned: as found, its real path, its version.
    #[tokio::test]
    async fn the_harness_row_carries_the_pinned_programs_path_settings_audit_r1() {
        let db = Store::open(wisp_store::Location::Memory, std::sync::Arc::new(wisp_store::SystemClock)).unwrap();
        install(&db).unwrap();
        let r = row_of(
            &HARNESS,
            &json!({ "id": "claude", "name": "Claude Code", "ord": 0, "available": true, "models": [], "signIn": "cli",
                     "pin": { "path": "/usr/local/bin/claude", "real": "/opt/claude/2.1/claude", "version": "2.1" } }),
        );
        db.write(move |w| w.upsert(HARNESS.kind, r).map(|_| ())).await.unwrap();
        let h = db.read(|x| x.get("accounts.harness", "claude")).unwrap().unwrap();
        assert_eq!(h["pin"]["real"], "/opt/claude/2.1/claude", "{h:?}");
        assert_eq!(h["pin"]["path"], "/usr/local/bin/claude");
    }

    /// QA r1 #23: Settings → Agents' GitHub row said nothing of its sign-in (the row had no field for it) while Home
    /// asked to sign in to GitHub. The tool row carries gh's `signedIn` (null: gh's answer unknown).
    #[tokio::test]
    async fn the_github_row_carries_whether_gh_is_signed_in() {
        let db = Store::open(wisp_store::Location::Memory, std::sync::Arc::new(wisp_store::SystemClock)).unwrap();
        install(&db).unwrap();
        let r = tool_row("gh", "GitHub CLI", &wisp_git_api::ToolState { installed: true, signed_in: Some(true), setup: None });
        db.write(move |w| w.upsert(TOOL.kind, r).map(|_| ())).await.unwrap();
        let t = db.read(|x| x.get("accounts.tool", "gh")).unwrap().unwrap();
        assert_eq!(t["signedIn"], true, "{t:?}");
        let r = tool_row("git", "git", &wisp_git_api::ToolState { installed: true, signed_in: None, setup: None });
        assert!(r.get("signedIn").is_none_or(serde_json::Value::is_null));
    }
}
