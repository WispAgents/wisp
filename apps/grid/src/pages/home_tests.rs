//! `home`'s tests (docs/ux/2026-11-18-grid.md #1–#5).
use super::*;

fn cfg() -> PageCfg {
    PageCfg { name: "wisp Cloud".into(), domain: "grid.trywisp.dev".into(), public_url: "https://grid.trywisp.dev".into() }
}

fn page() -> HomePage {
    HomePage {
        user: PageUser { handle: "ada".into(), name: Some("Ada Lovelace".into()), ..PageUser::default() },
        url: "https://ada.grid.trywisp.dev".into(),
        instance: InstanceView { state: "stopped".into(), ..InstanceView::default() },
        now_ms: 1_793_000_000_000,
        ..HomePage::default()
    }
}

#[test]
fn a_fresh_account_gets_one_machines_list_and_the_checklist() {
    let h = home_page(&cfg(), &page());
    assert!(h.contains("Hi Ada</h1>"));
    // the cloud wisp first, as a row with Open; no computer yet says so with the way to add one
    assert!(h.contains(r"<b>Cloud wisp</b>") && h.contains("Asleep · starts the first time you open it"), "{h}");
    assert!(h.contains(r#"<a class="primary" href="/go">Open"#));
    assert!(h.contains("No computers of yours yet") && h.contains("0 of 1 up"));
    assert!(h.contains(r#"data-copy="npx wisp-agents computer on""#));
    // asleep: no Sleep in the ⋯
    assert!(!h.contains("Sleep now"));
    // Getting started: nothing done, next is opening it; the rest folded at the end, no marketing tiles or picture
    assert!(h.contains("Getting started · 0 of 3 done") && h.contains("Next: open your cloud wisp"), "{h}");
    assert!(h.contains("What else can I do?") && h.contains(r#"href="/account#keys""#));
    assert!(!h.contains("What your cloud can do") && !h.contains("light-app.webp"));
    // no links section with nothing published or shared
    assert!(!h.contains(r#"id="apps""#) && !h.contains(r#"id="shares""#));
}

#[test]
fn running_with_computers_and_links() {
    let mut p = page();
    p.instance = InstanceView { state: "running".into(), started_at: Some(p.now_ms - 120_000), ..InstanceView::default() };
    p.computers = vec![
        ComputerView { id: "c1".into(), name: "mac".into(), url: "https://mac--ada.grid".into(), online: true, ..ComputerView::default() },
        ComputerView { id: "c2".into(), name: "<pc>".into(), last_seen_at: Some(p.now_ms - 3_600_000), ..ComputerView::default() },
        ComputerView { id: "c3".into(), name: "new".into(), ..ComputerView::default() },
    ];
    p.sites =
        vec![SiteView { key: "k".into(), title: "Demo".into(), url: "https://demo.grid/".into(), files: 1, updated_at: Some(p.now_ms) }];
    let h = home_page(&cfg(), &p);
    assert!(h.contains("Running · started 2 min ago"));
    assert!(h.contains("2 of 4 up"));
    assert!(h.contains("Sleep now"));
    // online: Open is the primary; offline, Open still goes, to the page that waits for it (#7)
    assert!(h.contains(r#"<a class="primary" href="/go?computer=c1">Open"#));
    assert!(h.contains(r#"<a class="ghost line" href="/go?computer=c2" title="It opens as soon as that computer connects">Open"#));
    assert!(h.contains("Offline · last seen 1 h ago"));
    assert!(h.contains("Offline · never connected · ") && h.contains("How to connect"));
    assert!(h.contains("&#60;pc&#62;"));
    assert!(h.contains(r#"action="/computers/c2/remove" data-confirm="Remove it?""#));
    // apps alone: one list, no tabs
    assert!(h.contains("Published apps <span class=\"n\">1</span>") && h.contains("updated just now · 1 file · public"), "{h}");
    assert!(!h.contains(r#"id="lnShares""#));
    // opened once: the first step is done
    assert!(h.contains("Getting started · 1 of 3 done") && h.contains("Next: sign in to an agent"));
}

#[test]
fn apps_and_shares_get_two_tabs_and_each_share_says_who_opens_it() {
    let mut p = page();
    p.sites = vec![SiteView { key: "k".into(), title: "Demo".into(), url: "https://demo.grid/".into(), files: 3, ..SiteView::default() }];
    p.shares = vec![
        ShareView {
            id: "s1".into(),
            title: "Fix it".into(),
            kind: "task".into(),
            url: "https://g/s/s1".into(),
            access: "password".into(),
            views: 1,
            ..ShareView::default()
        },
        ShareView {
            id: "s2".into(),
            title: "Old".into(),
            url: "https://g/s/s2".into(),
            access: "anyone".into(),
            views: 3,
            expires_at: Some(p.now_ms - 1),
            ..ShareView::default()
        },
    ];
    let h = home_page(&cfg(), &p);
    assert!(h.contains(
        r#"<label for="lnApps">Apps <span class="n">1</span></label><label for="lnShares">Shared tasks <span class="n">2</span></label>"#
    ));
    assert!(h.contains("task · with its passkey · 1 view · updated never"));
    assert!(h.contains("task · anyone with the link · 3 views · updated never · expired"));
    assert!(h.contains(r#"action="/shares/s1/revoke" data-confirm="Stop sharing?""#));
    assert!(h.contains(r#"data-copy="https://g/s/s1""#));
}

#[test]
fn the_checklist_ticks_from_its_cookie_and_goes_once_done_or_dismissed() {
    assert_eq!(Start::from_cookie("a, t"), Start { agent: true, task: true, dismissed: false });
    assert_eq!(Start::from_cookie(""), Start::default());
    let mut p = page();
    p.start = Start { agent: true, ..Start::default() };
    let h = home_page(&cfg(), &p);
    assert!(h.contains("Getting started · 1 of 3 done"));
    p.start.dismissed = true;
    assert!(!home_page(&cfg(), &p).contains("Getting started ·"));
    p = page();
    p.instance.started_at = Some(1);
    p.start = Start { agent: true, task: true, dismissed: false };
    assert!(!home_page(&cfg(), &p).contains("Getting started ·"));
}

#[test]
fn every_wisp_link_lands_on_its_place() {
    // the settings sheet as the wisp's URL reads it (packages/client/src/nav-url.ts): `?o=viewer:<id>:<json>`
    assert_eq!(
        wisp_settings("agents"),
        "/go?path=%2F%3Fo%3Dviewer%253Ashell-settings.sheet%253A%257B%2522pane%2522%253A%2522agents%2522%257D"
    );
    assert_eq!(wisp_at("/view/stuff.page"), "/go?path=%2Fview%2Fstuff.page");
    let h = home_page(&cfg(), &page());
    assert!(h.contains(&esc(&wisp_settings("agents"))) && h.contains(&esc(&wisp_settings("automations"))));
}

#[test]
fn add_offers_a_computer_and_the_cloud_machine_waitlist() {
    let h = home_page(&cfg(), &page());
    assert!(h.contains("A computer of yours") && h.contains("Another cloud machine"));
    assert!(h.contains(r#"<span class="pill soon">Waitlist</span>"#));
    assert!(!h.contains("Soon</span>"));
}
