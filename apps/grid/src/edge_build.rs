//! Making the grid: every part from its config (v1 grid/server.js `startGrid`).
use std::sync::Arc;

use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use serde_json::Value;

use super::{Grid, SignIn, route};
use crate::accounts::Accounts;
use crate::boxhost::BoxHost as _;
use crate::clock::SharedClock;
use crate::config::GridConfig;
use crate::db::Db;
use crate::hostd::ProcessHost;
use crate::hosts::root_hosts;
use crate::pages::PageCfg;
use crate::rows::BoxUser;
use crate::shares::Shares;
use crate::teams::Teams;
use crate::tunnels::Tunnels;

/// Share passwords hashed at once (scrypt, ~50 ms of CPU each): enough for real readers, few enough that guesses
/// can't take the blocking pool from everything else.
const SCRYPT_PERMITS: usize = 4;

impl Grid {
    pub fn new(cfg: GridConfig, db: Arc<Db>, clock: SharedClock, boxes: Arc<ProcessHost>) -> Arc<Self> {
        let telemetry = crate::telemetry::GridTelemetry::new(cfg.telemetry, &cfg.domain, &crate::config::process_env());
        Self::with_telemetry(cfg, db, clock, boxes, telemetry)
    }

    /// [`Grid::new`] with its usage stats made by the caller (a test sends them to its own `PostHog`).
    pub fn with_telemetry(
        cfg: GridConfig,
        db: Arc<Db>,
        clock: SharedClock,
        boxes: Arc<ProcessHost>,
        telemetry: crate::telemetry::GridTelemetry,
    ) -> Arc<Self> {
        let teams = Teams { db: db.clone(), clock: clock.clone() };
        let shares = Shares { db: db.clone(), clock: clock.clone(), teams: teams.clone(), public_url: cfg.public_url.clone() };
        let telemetry = Arc::new(telemetry);
        telemetry.run(boxes.changes(), clock.clone());
        let accounts = Accounts { db: db.clone(), clock: clock.clone(), telemetry: Some(telemetry.clone()) };
        let roots = root_hosts(&cfg.domain, &cfg.port_suffix, cfg.listen.port);
        let page = PageCfg::of(&cfg);
        let client = Client::builder(TokioExecutor::new()).build_http();
        let signin = SignIn::new(&cfg);
        // the master key seals a sandbox's token and sign-ins; without one, sandboxes are off and say why
        let secrets = crate::secrets::Secrets::open(db.clone(), &cfg.data_dir, &crate::config::process_env())
            .map(Arc::new)
            .map_err(|e| tracing::warn!("grid: no master key, so no sandboxes: {e}"))
            .ok();
        // media links outlive a restart when there's a master key; without one, a random key (they stop at restart)
        let media = crate::media::MediaKey::new(secrets.as_ref().and_then(|s| s.media_key().ok()).unwrap_or_else(|| {
            let mut k = [0u8; 32];
            k.copy_from_slice(&crate::secrets::random_bytes(32));
            k
        }));
        let github = github_of(&cfg, &clock);
        let sandboxes = sandboxes_of(&cfg, &db, &clock, secrets.clone(), &boxes, github.clone());
        let urls = cfg.clone();
        let sites = crate::sites::Sites::new(
            db.clone(),
            clock.clone(),
            &cfg.data_dir,
            cfg.file_cfg.sites.as_ref(),
            Arc::new(move |l: &str| urls.instance_url(l)),
        );
        let inbox = inbox_of(&cfg, &db, &clock, secrets.clone(), &boxes);
        let tunnels = Arc::new(Tunnels::default());
        let routes = routes_of(&cfg, &db, &clock, secrets.clone(), &tunnels, inbox.opts.enabled);
        let mail = mail_of(&cfg, &db, &clock, &inbox);
        if let Some(m) = mail.clone() {
            mail_threads(&inbox, m);
        }
        let keys = crate::keys::ApiKeys { db: db.clone(), clock: clock.clone() };
        let hooks = crate::hooks::Hooks { db: db.clone(), clock: clock.clone() };
        let held = Arc::new(crate::hook_held::HookHeld::new(db.clone(), clock.clone(), secrets.clone(), tunnels.clone()));
        let push = push_of(&cfg, &db, &clock);
        Arc::new(Self {
            cfg,
            db,
            clock,
            accounts,
            keys,
            hooks,
            held,
            limits: crate::http::Limiter::default(),
            scrypt: Arc::new(tokio::sync::Semaphore::new(SCRYPT_PERMITS)),
            fresh_keys: crate::keys_fresh::FreshKeys::default(),
            shares,
            teams,
            tunnels,
            boxes,
            roots,
            page,
            client,
            signin,
            telemetry,
            sandboxes,
            sites,
            inbox,
            routes,
            mail,
            push,
            github,
            notes: tokio::sync::broadcast::channel(64).0,
            media,
        })
    }
}

/// The inbox's settings from grid.json's `channels` (off unless `enabled: true`).
/// Mail, when grid.json has it, channels are held, and there's a master key (the addresses are sealed).
fn mail_of(
    cfg: &GridConfig,
    db: &Arc<crate::db::Db>,
    clock: &crate::clock::SharedClock,
    inbox: &crate::inbox::Inbox,
) -> Option<Arc<crate::mail::service::MailService>> {
    use crate::mail::service::{Http, MailService, Opts, Outbound};
    let m = cfg.file_cfg.mail.as_ref()?;
    if !inbox.opts.enabled || inbox.secrets.is_none() || m.inbound.provider != "ses" || m.outbound.provider != "ses" {
        return None;
    }
    let out = Outbound {
        region: m.outbound.region.clone(),
        access_key_id: m.outbound.access_key_id.clone(),
        secret_access_key: m.outbound.secret_access_key.clone().unwrap_or_default(),
        endpoint: m.outbound.extra.get("endpoint").and_then(Value::as_str).map(str::to_owned),
    };
    let opts = Opts {
        domain: m.domain.to_lowercase(),
        topic_arn: m.inbound.topic_arn.clone(),
        keys: m.inbound.keys.clone().unwrap_or_default(),
        outbound: out.clone(),
        send_per_hour: m.send_per_hour.unwrap_or(30),
        thread_days: m.thread_days.unwrap_or(7),
    };
    // a dev grid has no real senders, so grid.json can name their keys; nothing else skips DNS
    let txt: Arc<crate::dkim::Txt> = if let (true, Some(keys)) = (cfg.file_cfg.auth.dev, m.inbound.dkim_keys.clone()) {
        Arc::new(move |name: String| {
            let k = keys.get(&name).cloned();
            Box::pin(async move { k.map(|r| vec![vec![r]]).ok_or(crate::dkim::Lookup::NoSuchKey) })
        })
    } else {
        let Some(t) = crate::mail::service::system_txt() else {
            tracing::warn!("grid: mail: no DNS resolver, so no mail");
            return None;
        };
        t
    };
    let wire = Arc::new(Http {
        client: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().ok()?,
        out,
        clock: clock.clone(),
    });
    Some(Arc::new(MailService::new(db.clone(), clock.clone(), opts, txt, wire)))
}

fn inbox_opts(c: Option<&crate::config::ChannelsConfig>) -> crate::inbox::Opts {
    let d = crate::inbox::Opts::default();
    let Some(c) = c else { return d };
    crate::inbox::Opts {
        enabled: c.enabled == Some(true),
        max_rows: c.max_rows.unwrap_or(d.max_rows),
        max_bytes: c.max_bytes.unwrap_or(d.max_bytes),
        ttl_ms: c.ttl_ms.unwrap_or(d.ttl_ms),
        retry_ms: c.retry_ms.unwrap_or(d.retry_ms),
        wake_retry_ms: c.wake_retry_ms.unwrap_or(d.wake_retry_ms),
        ..d
    }
}

type Sealed = Option<Arc<crate::secrets::Secrets>>;

/// Sandboxes, and the env and sign-ins their boxes start with (a sandbox's box gets its owner's, a hub's box its team's;
/// a person's own box never).
/// The GitHub App grid.json names (its secrets read now): a grid whose App can't be read says why and runs without it.
fn github_of(cfg: &GridConfig, clock: &SharedClock) -> Option<Arc<crate::github_app::GitHubApp>> {
    match crate::github_app::GitHubApp::from_config(cfg.file_cfg.github_app.as_ref(), &crate::config::process_env(), clock.clone()) {
        Ok(app) => app.map(Arc::new),
        Err(e) => {
            tracing::error!("grid: the GitHub App is off: {e}");
            None
        }
    }
}

type App = Option<Arc<crate::github_app::GitHubApp>>;

fn sandboxes_of(
    cfg: &GridConfig,
    db: &Arc<Db>,
    clock: &SharedClock,
    secrets: Sealed,
    boxes: &ProcessHost,
    github: App,
) -> crate::sandboxes::Sandboxes {
    let sb_cfg = cfg.sandboxes.clone().unwrap_or_default();
    let sandboxes = crate::sandboxes::Sandboxes {
        db: db.clone(),
        clock: clock.clone(),
        secrets,
        enabled: sb_cfg.enabled != Some(false),
        per_user: sb_cfg.per_user.unwrap_or(10),
    };
    let sb = sandboxes.clone();
    let teams = crate::teams::Teams { db: db.clone(), clock: clock.clone() };
    // a team's GitHub App installation (ADR-0148 §8e), when the hub's team connected one
    let connected = {
        let (teams, github) = (teams.clone(), github.clone());
        move |hub_user: &str| -> Option<i64> {
            github.as_ref()?;
            let hub = teams.hub_by_user(hub_user).ok().flatten()?;
            teams.github_of(&hub.team_id).ok().flatten().map(|c| c.installation_id)
        }
    };
    boxes.set_env_for(Arc::new(move |u: &BoxUser| {
        if let Some(o) = u.owner.as_deref().filter(|_| u.sandbox.is_some()) {
            let mut env = sb.env_for(o);
            // a hub's sandbox, with GitHub connected: the App's token for its job (an hour; a job is shorter), in place of
            // a GH_TOKEN secret
            if let Some(t) = connected(o).and_then(|id| github.as_ref().and_then(|a| a.cached(id))) {
                env.retain(|(k, _)| k != "GH_TOKEN" && k != "GITHUB_TOKEN");
                env.push(("GH_TOKEN".into(), t.clone()));
                env.push(("GITHUB_TOKEN".into(), t));
            }
            return env;
        }
        // a team's hub box starts with its team's secrets (ADR-0148 §6); a person's own box never with sandboxes'
        // and says it's a hub, so it keeps the plugins its sandboxes need (`HubPlugins`)
        if let Ok(Some(hub)) = teams.hub_by_user(&u.id) {
            let mut env = sb.hub_env(&u.id);
            // what its people see it called, there and as a machine elsewhere ("Acme hub", grid-client's hub_peers)
            let name = if hub.name.trim().is_empty() { hub.slug.as_str() } else { hub.name.trim() };
            env.push(("WISP_NAME".into(), format!("{name} hub")));
            // its team's secrets answer as ${secret:NAME} there (wisp's FileSecrets): their names, never more
            let names: Vec<String> = sb.env_keys(&u.id);
            env.push(("WISP_HUB_SECRETS".into(), names.join(",")));
            env.push(("WISP_HUB".into(), "1".into()));
            // with GitHub connected its gh and git use the App's tokens, which it renews itself (grid-client): a
            // GH_TOKEN in its env would win over them and stop working within the hour
            if connected(&u.id).is_some() {
                env.retain(|(k, _)| k != "GH_TOKEN" && k != "GITHUB_TOKEN");
            }
            env
        } else {
            Vec::new()
        }
    }));
    sandboxes
}

/// `on laptop` through the grid's own bot: on with held channels and `channels.route`.
fn routes_of(
    cfg: &GridConfig,
    db: &Arc<Db>,
    clock: &SharedClock,
    secrets: Sealed,
    tunnels: &Arc<Tunnels>,
    held: bool,
) -> crate::route::Routes {
    let ch = cfg.file_cfg.channels.clone().unwrap_or_default();
    crate::route::Routes {
        db: db.clone(),
        clock: clock.clone(),
        secrets,
        telegram: Arc::new(crate::route::HttpTelegram {
            base: ch.telegram_api.clone().unwrap_or_else(|| "https://api.telegram.org".into()),
            client: reqwest::Client::new(),
        }),
        computers: Arc::new(route::TunnelComputers(tunnels.clone())),
        public_url: cfg.public_url.clone(),
        on: held && ch.route == Some(true),
    }
}

/// A mail the box took keeps its thread, so the box may answer it.
fn mail_threads(inbox: &crate::inbox::Inbox, m: Arc<crate::mail::service::MailService>) {
    let _ = inbox.delivered.set(Arc::new(move |user: &str, d: &crate::inbox::Delivery| {
        if d.channel == "email"
            && let Err(e) = m.delivered(user, &d.headers, &d.body)
        {
            tracing::warn!("grid: mail: a thread wasn’t kept: {e}");
        }
    }));
}

/// The phone relay, with its providers when grid.json names them.
fn push_of(cfg: &GridConfig, db: &Arc<Db>, clock: &SharedClock) -> crate::push::Relay {
    let push_cfg = cfg.file_cfg.push.clone();
    let sender = crate::push::providers::HttpPush::from_config(push_cfg.as_ref(), &crate::config::process_env())
        .map(|p| Arc::new(p) as Arc<dyn crate::push::PushSender>);
    crate::push::Relay::new(db.clone(), clock.clone(), sender, push_cfg.and_then(|c| c.per_minute))
}

/// Channels' messages held for sleeping boxes.
fn inbox_of(cfg: &GridConfig, db: &Arc<Db>, clock: &SharedClock, secrets: Sealed, boxes: &Arc<ProcessHost>) -> crate::inbox::Inbox {
    crate::inbox::Inbox::new(
        db.clone(),
        clock.clone(),
        secrets,
        boxes.clone(),
        Arc::new(crate::inbox::HttpDeliver::default()),
        inbox_opts(cfg.file_cfg.channels.as_ref()),
        {
            let urls = cfg.clone();
            Arc::new(move |h: &str| urls.instance_url(h))
        },
    )
}
