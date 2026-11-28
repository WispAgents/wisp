//! What the Teams pages show, and to whom (ADR-0147).
use super::*;

fn cfg() -> PageCfg {
    PageCfg { name: "wisp Grid".into(), public_url: "https://grid.test".into(), ..PageCfg::default() }
}

fn hub(role: &str) -> TeamPage {
    TeamPage {
        user: PageUser { handle: "ada".into(), ..PageUser::default() },
        slug: "acme".into(),
        name: "Acme".into(),
        url: "https://acme.grid.test".into(),
        state: "running".into(),
        role: role.into(),
        members: vec![MemberRow { handle: "ada".into(), role: role.into() }, MemberRow { handle: "bo".into(), role: "member".into() }],
        keys: vec![HubKeyRow {
            id: "0a1b".into(),
            name: "ci-main".into(),
            hint: "my80".into(),
            rights: "deploy".into(),
            ..HubKeyRow::default()
        }],
        env: vec!["GH_TOKEN".into()],
        logins: vec![("claude".into(), false)],
        sealed: true,
        deploys: vec![DeployRow {
            n: 2,
            by: "ci-main".into(),
            source: "acme/agents@a1b2c3".into(),
            state: "done".into(),
            counts: (2, 1, 0),
            ..DeployRow::default()
        }],
        csrf: "tok".into(),
        now_ms: 1_793_000_000_000,
        ..TeamPage::default()
    }
}

#[test]
fn an_owner_runs_the_hub_and_a_member_sees_its_people_and_deploys_only() {
    let owner = team_page(&cfg(), &hub("owner"));
    for part in [
        r#"href="/go?hub=acme""#,
        r#"action="/teams/acme/members""#,
        r#"action="/teams/acme/invites""#,
        r#"action="/teams/acme/keys""#,
        r#"action="/teams/acme/secrets""#,
        r#"action="/teams/acme/delete""#,
        "ci-main",
        "GH_TOKEN",
        "acme/agents@a1b2c3",
        "+2 ~1 −0",
        r#"aria-current="page">Teams"#,
    ] {
        assert!(owner.contains(part), "an owner's page has {part}");
    }
    let member = team_page(&cfg(), &hub("member"));
    for part in [r#"href="/go?hub=acme""#, "acme/agents@a1b2c3", "Leave"] {
        assert!(member.contains(part), "a member's page has {part}");
    }
    for part in ["/teams/acme/keys", "/teams/acme/secrets", "/teams/acme/invites", "/teams/acme/delete", "GH_TOKEN", "/members/bo/remove"] {
        assert!(!member.contains(part), "a member's page has no {part}");
    }
    // an admin manages, but deletes nothing and makes no owners
    let admin = team_page(&cfg(), &hub("admin"));
    assert!(admin.contains("/teams/acme/keys") && !admin.contains("/teams/acme/delete") && !admin.contains(r#"<option value="owner""#));
}

#[test]
fn a_key_or_a_link_just_made_is_said_once_and_what_people_typed_is_escaped() {
    let mut p = hub("owner");
    p.fresh = Some(("ci-main".into(), "wgh_secret".into()));
    let page = team_page(&cfg(), &p);
    assert!(page.contains("wgh_secret") && page.contains("gh secret set WISP_HUB_KEY"));
    p.fresh = Some(("invite".into(), "https://grid.test/join/wgi_x".into()));
    let page = team_page(&cfg(), &p);
    assert!(page.contains("https://grid.test/join/wgi_x") && !page.contains("wgh_secret"));
    p.name = "<script>x</script>".into();
    assert!(!team_page(&cfg(), &p).contains("<script>x</script>"));
    let list = teams_page(
        &cfg(),
        &TeamsPage {
            teams: vec![TeamRow {
                slug: "acme".into(),
                name: "Acme".into(),
                role: "owner".into(),
                url: "https://acme.grid.test".into(),
                state: "stopped".into(),
            }],
            ..TeamsPage::default()
        },
    );
    assert!(list.contains(r#"href="/teams/acme""#) && list.contains("acme.grid.test") && list.contains(r#"action="/teams""#));
}
