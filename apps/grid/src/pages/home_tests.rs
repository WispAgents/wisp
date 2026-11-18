//! `home`'s tests.
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
fn a_fresh_account_gets_the_first_steps() {
    let h = home_page(&cfg(), &page());
    assert!(h.contains("Hi Ada</h1>"));
    assert!(h.contains("Open your cloud wisp"));
    assert!(h.contains("Starts the first time you open it"));
    assert!(h.contains("What your cloud can do"));
    assert!(h.contains(r#"data-copy="npx wisp-agents computer on""#));
    assert!(h.contains(r#"<form class="inline off" method="post" action="/instance/stop">"#));
}

#[test]
fn computers_and_apps_show_as_cards() {
    let mut p = page();
    p.instance = InstanceView { state: "running".into(), started_at: Some(p.now_ms - 120_000), ..InstanceView::default() };
    p.computers = vec![
        ComputerView { id: "c1".into(), name: "mac".into(), url: "https://mac--ada.grid".into(), online: true, ..ComputerView::default() },
        ComputerView { id: "c2".into(), name: "<pc>".into(), last_seen_at: Some(p.now_ms - 3_600_000), ..ComputerView::default() },
    ];
    p.sites =
        vec![SiteView { key: "k".into(), title: "Demo".into(), url: "https://demo.grid/".into(), files: 1, updated_at: Some(p.now_ms) }];
    let h = home_page(&cfg(), &p);
    assert!(!h.contains("Open your cloud wisp</b>"));
    assert!(h.contains("Started 2 min ago."));
    assert!(h.contains("1 of 2 online"));
    assert!(h.contains("Offline · last seen 1 h ago"));
    assert!(h.contains("&#60;pc&#62;"));
    assert!(h.contains("Updated just now · 1 file</small>"));
    assert!(h.contains("1 published"));
    assert!(h.contains("2 added"));
}
