//! What the grid client keeps (ADR-0004: one write path): one table, `grid_client_doc`, on the
//! `plugin:wisp.grid-client:account` topic.
//! - `account:<grid>#<handle>`: one per account signed in to (ADR-0061): the grid, as whom, its cloud box's peer,
//!   and this computer on that grid;
//! - `current`: `{key}`, the account shares, sandboxes, keys and this computer's tunnel go through;
//! - `import:<share id>`: a share taken in here (an import, a module source) and the version taken, so
//!   `grid-client.updates` can say "update available".
//!
//! Each account's device token goes to secrets (`grid-client/token/<grid>#<handle>`), never here. Migration 2
//! turns the one-account shape (`account`) into this one; `St::load` moves its token (`grid-client/token`).
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_grid_client_api::{AccountInfo, ComputerState, SignIn};
use wisp_kernel::{CancellationToken, Ctx, Result, WireError};
use wisp_store::{Col, EntityDef, Migration, Query, Row, Store};

pub const OWNER: &str = "grid-client";
pub const TOPIC: &str = "plugin:wisp.grid-client:account";
/// The one-account shape's token (before ADR-0061): moved to `token_key` at start.
pub const OLD_TOKEN: &str = "grid-client/token";
pub const CURRENT: &str = "current";

pub const MIGRATIONS: &[Migration] = &[
    Migration::sql(1, "CREATE TABLE grid_client_doc (id TEXT PRIMARY KEY, body TEXT NOT NULL) STRICT;"),
    // ADR-0061: the one `account` becomes `account:<grid>#<handle>`, and current
    Migration::sql(
        2,
        "INSERT OR REPLACE INTO grid_client_doc (id, body)
           SELECT 'account:' || json_extract(body, '$.grid') || '#' || json_extract(body, '$.handle'), body
           FROM grid_client_doc WHERE id = 'account' AND json_valid(body) AND json_extract(body, '$.grid') <> '' AND json_extract(body, '$.handle') <> '';
         INSERT OR REPLACE INTO grid_client_doc (id, body)
           SELECT 'current', json_object('key', json_extract(body, '$.grid') || '#' || json_extract(body, '$.handle'))
           FROM grid_client_doc WHERE id = 'account' AND json_valid(body) AND json_extract(body, '$.grid') <> '' AND json_extract(body, '$.handle') <> '';
         DELETE FROM grid_client_doc WHERE id = 'account';",
    ),
];

/// An account's key: `<grid>#<handle>` (a handle never holds `#`; two accounts on one grid differ by it).
pub fn key_of(grid: &str, handle: &str) -> String {
    format!("{grid}#{handle}")
}

/// Where an account's device token is in secrets.
pub fn token_key(key: &str) -> String {
    format!("{OLD_TOKEN}/{key}")
}

fn on_topic(_: &Row) -> Vec<String> {
    vec![TOPIC.into()]
}

pub const DOC: EntityDef = EntityDef {
    kind: "grid_client_doc",
    table: "grid_client_doc",
    key: "id",
    columns: &[("id", Col::Text), ("body", Col::Text)],
    json_columns: &["body"],
    parent: None,
    seq: None,
    topics: on_topic,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub grid: String,
    pub handle: String,
    /// the cloud box's peer id in machines
    #[serde(default)]
    pub machine: String,
    /// this computer on the grid, once added
    #[serde(default)]
    pub computer_id: String,
    #[serde(default)]
    pub computer_label: String,
    #[serde(default)]
    pub computer_name: String,
    #[serde(default)]
    pub computer_on: bool,
    /// when it was signed in to here (ms): the order they're listed in
    #[serde(default)]
    pub added: i64,
}

impl Account {
    pub fn key(&self) -> String {
        key_of(&self.grid, &self.handle)
    }
}

/// Every account signed in to here and which is current (sans-IO: the rules, no store, no secrets).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Accounts {
    /// in the order they were signed in to
    pub list: Vec<Account>,
    /// the current one's key ("" when there's none)
    pub current: String,
}

impl Accounts {
    /// From what the store held: `current` is kept only if it names one of them, else the first is.
    pub fn new(mut list: Vec<Account>, current: &str) -> Self {
        list.sort_by(|a, b| a.added.cmp(&b.added).then_with(|| a.key().cmp(&b.key())));
        list.dedup_by_key(|a| a.key());
        let current =
            if list.iter().any(|a| a.key() == current) { current.to_owned() } else { list.first().map(Account::key).unwrap_or_default() };
        Self { list, current }
    }

    pub fn current(&self) -> Option<&Account> {
        self.list.iter().find(|a| a.key() == self.current)
    }

    pub fn get(&self, key: &str) -> Option<&Account> {
        self.list.iter().find(|a| a.key() == key)
    }

    /// The account to use on `grid`: the current one if it's there, else the first signed in to there.
    pub fn on_grid(&self, grid: &str) -> Option<&Account> {
        self.current().filter(|a| a.grid == grid).or_else(|| self.list.iter().find(|a| a.grid == grid))
    }

    /// Added (or, the same grid and handle again, replaced in its place) and made current.
    pub fn add(&mut self, a: Account) {
        self.current = a.key();
        self.put(a);
    }

    /// Replaced in its place, or added at the end; current stays.
    pub fn put(&mut self, a: Account) {
        match self.list.iter_mut().find(|x| x.key() == a.key()) {
            Some(x) => *x = a,
            None => self.list.push(a),
        }
    }

    /// Replaced in its place if it's still signed in to (a change that lands after a sign-out is dropped).
    pub fn update(&mut self, a: Account) -> bool {
        self.list.iter_mut().find(|x| x.key() == a.key()).map(|x| *x = a).is_some()
    }

    /// Whether `key` is one of them (and now current).
    pub fn switch(&mut self, key: &str) -> bool {
        let known = self.get(key).is_some();
        if known {
            key.clone_into(&mut self.current);
        }
        known
    }

    /// Taken out; if it was current, the first left is current (none when it was the last).
    pub fn remove(&mut self, key: &str) -> Option<Account> {
        let i = self.list.iter().position(|a| a.key() == key)?;
        let gone = self.list.remove(i);
        if self.current == key {
            self.current = self.list.first().map(Account::key).unwrap_or_default();
        }
        Some(gone)
    }

    pub fn infos(&self) -> Vec<AccountInfo> {
        self.list
            .iter()
            .map(|a| AccountInfo {
                key: a.key(),
                grid: a.grid.clone(),
                handle: a.handle.clone(),
                machine: a.machine.clone(),
                current: a.key() == self.current,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    pub id: String,
    pub grid: String,
    pub url: String,
    pub title: String,
    pub kind: String,
    pub version: i64,
}

pub struct St {
    pub ctx: Ctx,
    pub store: Store,
    pub http: reqwest::Client,
    pub accounts: Mutex<Accounts>,
    pub flows: Mutex<HashMap<String, SignIn>>,
    pub computer: Mutex<ComputerState>,
    pub dialer: Mutex<Option<CancellationToken>>,
    /// the mark on requests that came through this computer's tunnel (this process only)
    pub mark: String,
}

impl std::fmt::Debug for St {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("St").finish_non_exhaustive()
    }
}

pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn hex(n: usize) -> String {
    (0..n).fold(String::with_capacity(n * 2), |mut out, _| {
        let _ = write!(out, "{:02x}", rand::random::<u8>());
        out
    })
}

pub fn install(store: &Store) -> wisp_store::Result<()> {
    store.migrate(OWNER, MIGRATIONS)?;
    store.define_entity(DOC)
}

fn doc(id: &str, body: &impl Serialize) -> wisp_store::Result<Row> {
    let mut r = Row::new();
    r.insert("id".into(), json!(id));
    r.insert("body".into(), serde_json::to_value(body)?);
    Ok(r)
}

impl St {
    pub async fn load(ctx: Ctx, store: Store) -> Result<Arc<Self>> {
        let docs = store.read_async(|r| r.query(DOC.kind, &Query::new())).await.map_err(|e| WireError::internal(e.to_string()))?;
        let id = |d: &Row| d.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        let list = docs
            .iter()
            .filter(|d| id(d).starts_with("account:"))
            .filter_map(|d| d.get("body").cloned())
            .filter_map(|b| serde_json::from_value::<Account>(b).ok())
            .collect();
        let current = docs
            .iter()
            .find(|d| id(d) == CURRENT)
            .and_then(|d| d.get("body").and_then(|b| b.get("key")).and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        let accounts = Accounts::new(list, &current);
        move_old_token(&ctx, &accounts).await;
        Ok(Arc::new(Self {
            ctx,
            store,
            http: crate::net::client(false),
            accounts: Mutex::new(accounts),
            flows: Mutex::new(HashMap::new()),
            computer: Mutex::new(ComputerState { state: "off".into(), ..ComputerState::default() }),
            dialer: Mutex::new(None),
            mark: hex(24),
        }))
    }

    /// The current account.
    pub fn account(&self) -> Option<Account> {
        lock(&self.accounts).current().cloned()
    }

    pub fn accounts(&self) -> Accounts {
        lock(&self.accounts).clone()
    }

    /// Writes what changed between `before` and the accounts now (each account's doc, and current), in one write.
    async fn persist(&self, before: &Accounts) -> Result<()> {
        let now = self.accounts();
        let (put, gone): (Vec<Account>, Vec<String>) = (
            now.list.iter().filter(|a| before.get(&a.key()) != Some(*a)).cloned().collect(),
            before.list.iter().map(Account::key).filter(|k| now.get(k).is_none()).collect(),
        );
        let current = (now.current != before.current).then(|| now.current.clone());
        if put.is_empty() && gone.is_empty() && current.is_none() {
            return Ok(());
        }
        self.store
            .write(move |w| {
                for a in &put {
                    w.upsert(DOC.kind, doc(&format!("account:{}", a.key()), a)?)?;
                }
                for k in &gone {
                    w.del(DOC.kind, format!("account:{k}"))?;
                }
                match current.as_deref() {
                    Some("") => w.del(DOC.kind, CURRENT).map(|_| ())?,
                    Some(k) => w.upsert(DOC.kind, doc(CURRENT, &json!({ "key": k }))?).map(|_| ())?,
                    None => {}
                }
                Ok(())
            })
            .await
            .map_err(|e| WireError::internal(e.to_string()))
    }

    /// Changes the accounts (`f` returns what to answer) and writes the change.
    pub async fn change<T>(&self, f: impl FnOnce(&mut Accounts) -> T) -> Result<T> {
        let (before, out) = {
            let mut g = lock(&self.accounts);
            let before = g.clone();
            (before, f(&mut g))
        };
        if let Err(e) = self.persist(&before).await {
            // not written: back to what the store holds, so the next change writes from there
            *lock(&self.accounts) = before;
            return Err(e);
        }
        Ok(out)
    }

    /// An account, saved with this change (its computer, its cloud box), if it's still signed in to.
    pub async fn save_account(&self, a: Account) -> Result<()> {
        self.change(|x| x.update(a)).await.map(|_| ())
    }

    /// The current account's token.
    pub async fn token(&self) -> Option<String> {
        let key = lock(&self.accounts).current.clone();
        self.token_for(&key).await
    }

    pub async fn token_for(&self, key: &str) -> Option<String> {
        if key.is_empty() {
            return None;
        }
        wisp_kernel::services::secrets(&self.ctx).ok()?.get(&token_key(key)).await.ok().flatten().filter(|t| !t.is_empty())
    }

    /// The current account and its token, or the words to sign in.
    pub async fn signed_in(&self) -> std::result::Result<(Account, String), String> {
        let a = self.account().ok_or_else(|| "Not signed in to wisp Cloud: wisp cloud signin".to_owned())?;
        let t = self.token_for(&a.key()).await.ok_or_else(|| "Not signed in to wisp Cloud: wisp cloud signin".to_owned())?;
        Ok((a, t))
    }

    /// Every account with its token (one whose token is gone is left out).
    pub async fn signed_in_all(&self) -> Vec<(Account, String)> {
        let mut out = Vec::new();
        for a in self.accounts().list {
            if let Some(t) = self.token_for(&a.key()).await {
                out.push((a, t));
            }
        }
        out
    }

    pub async fn imports(&self) -> Result<Vec<Import>> {
        let docs = self.store.read_async(|r| r.query(DOC.kind, &Query::new())).await.map_err(|e| WireError::internal(e.to_string()))?;
        Ok(docs
            .iter()
            .filter(|d| d.get("id").and_then(Value::as_str).is_some_and(|i| i.starts_with("import:")))
            .filter_map(|d| d.get("body").cloned())
            .filter_map(|b| serde_json::from_value(b).ok())
            .collect())
    }

    pub async fn save_import(&self, i: Import) -> Result<()> {
        self.store
            .write(move |w| w.upsert(DOC.kind, doc(&format!("import:{}", i.id), &i)?).map(|_| ()))
            .await
            .map_err(|e| WireError::internal(e.to_string()))
    }

    pub fn new_flow_id() -> String {
        hex(6)
    }
}

/// The one-account shape's token (`grid-client/token`) becomes the migrated account's own, once.
async fn move_old_token(ctx: &Ctx, accounts: &Accounts) {
    let Ok(sec) = wisp_kernel::services::secrets(ctx) else { return };
    let Some(old) = sec.get(OLD_TOKEN).await.ok().flatten() else { return };
    // the migrated account is the only one there can be next to an old token
    let Some(a) = accounts.current().filter(|_| accounts.list.len() == 1) else { return };
    let new = token_key(&a.key());
    if sec.get(&new).await.ok().flatten().is_none() && sec.set(&new, &old).await.is_err() {
        return;
    }
    if let Err(e) = sec.delete(OLD_TOKEN).await {
        tracing::warn!(error = %e.message, "grid-client: the old token");
    }
}
