//! The v2 stream on a real store: snapshots from its tables, deltas from its change feed, and every
//! commit reaching an open stream (`Store::on_commit` → `Driver::committed`), as `wisp serve` wires it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::sync::Arc;
use std::time::Duration;

use support::{Fake, config, json_h, send};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wisp_api::v2::{Driver, PageSink, Snapshots};
use wisp_api::v2_store::{Live, PagedSource, StoreSnapshots, TopicReader, TopicSource, feed, live_changes, to_wire};
use wisp_api::{Edge, Hub};
use wisp_protocol::{Change, PeerInfo, Topic};
use wisp_store::{Location, ManualClock, Query, Store, row};

fn peer() -> PeerInfo {
    PeerInfo {
        peer: "laptop".into(),
        name: "laptop".into(),
        build: "test".into(),
        version: "2.0.0".into(),
        proto_min: 2,
        proto_max: 2,
        plugins: vec![],
    }
}

fn sources() -> Vec<TopicSource> {
    vec![TopicSource { kind: "setting", key: "key", matches: |t| t == "config", query: |_| Query::new() }]
}

fn store() -> Store {
    Store::open(Location::Memory, Arc::new(ManualClock::new(1_000))).unwrap()
}

async fn set(s: &Store, key: &'static str, value: &'static str) {
    s.write(move |w| {
        w.upsert("setting", row!({"key": key, "value": value, "locked_by": null}))?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn snapshot_then_delta_from_the_store() {
    let s = store();
    set(&s, "theme", "dark").await;
    let snaps = StoreSnapshots::new(s.clone(), sources(), Vec::new());
    let config = Topic::parse("config").unwrap();
    let full = snaps.snapshot(&config, None).await;
    assert!(full.full);
    assert_eq!(full.rows.len(), 1);
    assert_eq!((full.rows[0].e.as_str(), full.rows[0].id.as_str()), ("setting", "theme"));
    assert_eq!(snaps.head_seq(), full.seq, "the head follows the commits");

    set(&s, "volume", "3").await;
    set(&s, "theme", "light").await;
    let delta = snaps.snapshot(&config, Some(full.seq)).await;
    assert!(!delta.full);
    let ids: Vec<&str> = delta.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["volume", "theme"], "each row once, at its latest change");
    assert_eq!(delta.rows[1].v["value"], "light");
    assert_eq!(snaps.head_seq(), delta.seq);

    // a topic no source names reads from the change feed
    let other = snaps.snapshot(&Topic::parse("list").unwrap(), None).await;
    assert!(other.full && other.rows.is_empty());
}

/// A plugin's own first-look read (wisp.runtime's task window): it answers the full snapshot in place of the
/// change feed; a resume still reads only what changed.
#[tokio::test]
async fn a_topic_reader_answers_the_first_look_and_a_resume_reads_the_feed() {
    let s = store();
    set(&s, "theme", "dark").await;
    let snaps = StoreSnapshots::new(s.clone(), sources(), Vec::new()).with_readers(vec![TopicReader {
        matches: |t| t == "config",
        read: |_, _| Ok(vec![("setting".into(), "windowed".into(), serde_json::Map::new())]),
        kinds: &["setting"],
        live: None,
    }]);
    let config = Topic::parse("config").unwrap();
    let full = snaps.snapshot(&config, None).await;
    assert!(full.full);
    assert_eq!(full.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["windowed"], "the reader wins over the source");
    set(&s, "volume", "3").await;
    let delta = snaps.snapshot(&config, Some(full.seq)).await;
    assert!(!delta.full);
    assert_eq!(delta.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["volume"]);
}

/// Another plugin's kind on a reader's topic (wisp.goal's checks on `session:<id>`) comes in full beside the reader's
/// rows on a first look; a kind the reader answers for never comes twice.
#[tokio::test]
async fn a_first_look_brings_other_kinds_on_a_readers_topic_once() {
    let s = store();
    set(&s, "theme", "dark").await;
    let snaps = StoreSnapshots::new(s.clone(), sources(), Vec::new()).with_readers(vec![TopicReader {
        matches: |t| t == "config",
        read: |_, _| Ok(vec![("turn".into(), "t1".into(), serde_json::Map::new())]),
        kinds: &["turn"],
        live: None,
    }]);
    let full = snaps.snapshot(&Topic::parse("config").unwrap(), None).await;
    let ids: Vec<(&str, &str)> = full.rows.iter().map(|r| (r.e.as_str(), r.id.as_str())).collect();
    assert_eq!(ids, [("turn", "t1"), ("setting", "theme")]);
}

#[tokio::test]
async fn commits_become_wire_changes() {
    let s = store();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let _sub = s.on_commit(move |rows| {
        let _ = tx.send(rows.iter().flat_map(to_wire).collect::<Vec<_>>());
    });
    set(&s, "theme", "dark").await;
    set(&s, "theme", "light").await;
    let first = rx.recv().await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].topic.as_str(), "config");
    assert!(matches!(&first[0].change, Change::Upsert { v } if v["value"] == "dark"));
    let second = rx.recv().await.unwrap();
    assert!(matches!(&second[0].change, Change::Patch { ops } if !ops.is_empty()), "an update is a patch");
    assert!(second[0].seq > first[0].seq);
}

async fn until(c: &mut tokio::net::TcpStream, seen: &mut String, want: &str) {
    let mut buf = vec![0u8; 8192];
    let end = tokio::time::Instant::now() + Duration::from_secs(3);
    while !seen.contains(want) {
        let n = tokio::time::timeout_at(end, c.read(&mut buf)).await.unwrap_or_else(|_| panic!("no {want} in {seen}")).unwrap();
        assert!(n > 0, "stream closed before {want}: {seen}");
        seen.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
}

#[tokio::test]
async fn a_commit_reaches_an_open_stream() {
    let s = store();
    set(&s, "theme", "dark").await;
    let fake = Arc::new(Fake { calls: std::sync::atomic::AtomicUsize::new(0), live: 0 });
    let hub = Hub::new();
    let driver = Driver::start(Arc::new(StoreSnapshots::new(s.clone(), sources(), Vec::new())), fake.clone(), peer());
    let _feed = feed(&s, driver.clone());
    hub.mount(driver);
    let server =
        wisp_api::serve(Edge::new(config(), fake, hub), tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap()).await.unwrap();
    let p = server.addr.port();
    let mut c = tokio::net::TcpStream::connect(("127.0.0.1", p)).await.unwrap();
    let q = "proto_min=2&proto_max=2&client_id=c1&build=t";
    c.write_all(format!("GET /api/events?{q} HTTP/1.1\r\nhost: 127.0.0.1:{p}\r\n\r\n").as_bytes()).await.unwrap();
    let mut seen = String::new();
    until(&mut c, &mut seen, "welcome").await;
    assert!(seen.contains(&format!("\"epoch\":\"{}\"", s.epoch())), "the store's epoch: {seen}");
    let sub = send(p, "POST", "/api/events", &[json_h(), ("x-wisp-client", "c1")], br#"{"t":"sub","topic":"config"}"#).await;
    assert_eq!(sub.status, 200);
    until(&mut c, &mut seen, "\"theme\"").await;
    set(&s, "volume", "3").await;
    until(&mut c, &mut seen, "\"volume\"").await;
    server.shutdown().await;
}

/// A reader's [`Live`] (the task window's: a change to a stubbed turn goes as the stub): a committed change of
/// one of its kinds goes as the upserts it gives, in their order, on their topics; a resume's rows go through
/// `delta`; another kind goes as committed.
fn stubbing() -> TopicReader {
    TopicReader {
        matches: |t| t == "config",
        read: |_, _| Ok(Vec::new()),
        kinds: &["setting"],
        live: Some(Live {
            kinds: &["setting"],
            change: |r, topic, _, id| {
                let Some(mut v) = r.get("setting", id)? else { return Ok(None) };
                v.insert("value".into(), "stubbed".into());
                Ok(Some(vec![(topic.to_owned(), id.to_owned(), v.clone()), ("list".into(), id.to_owned(), v)]))
            },
            delta: |_, _, rows| {
                for (_, v) in rows {
                    v.insert("value".into(), "stubbed".into());
                }
                Ok(())
            },
        }),
    }
}

#[tokio::test]
async fn a_live_reader_sends_a_change_as_its_rows_and_a_resume_through_its_delta() {
    let s = store();
    set(&s, "theme", "dark").await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let _tap = s.on_commit(move |rows| {
        let _ = tx.send(rows.iter().flat_map(to_wire).collect::<Vec<_>>());
    });
    let snaps = StoreSnapshots::new(s.clone(), sources(), Vec::new()).with_readers(vec![stubbing()]);
    let head = snaps.head_seq();
    set(&s, "theme", "light").await;
    let committed = rx.recv().await.unwrap();
    assert!(matches!(&committed[0].change, Change::Patch { .. }), "the store committed a diff");
    let seq = committed[0].seq;
    let sent = live_changes(&s, &[stubbing()], committed);
    let got: Vec<(&str, &str)> = sent.iter().map(|c| (c.topic.as_str(), c.id.as_str())).collect();
    assert_eq!(got, [("config", "theme"), ("list", "theme")], "its rows, in their order, on their topics");
    assert!(sent.iter().all(|c| c.seq == seq && matches!(&c.change, Change::Upsert { v } if v["value"] == "stubbed")));
    let untouched = vec![wisp_sync::ChangeRow {
        seq,
        topic: Topic::parse("config").unwrap(),
        entity: "other".into(),
        id: "x".into(),
        change: Change::Del,
        at: 0,
    }];
    assert_eq!(live_changes(&s, &[stubbing()], untouched.clone()), untouched, "another kind, or a del, goes as committed");
    let resumed = snaps.snapshot(&Topic::parse("config").unwrap(), Some(head)).await;
    assert!(!resumed.full);
    assert_eq!(resumed.rows[0].v["value"], "stubbed", "a resume goes through delta");
}

// ---------- a first look in pages (ADR-0081) ----------

/// Settings paged as wisp.runtime's tasks are on `list`: the `a` values first, then the `b` ones, each by key.
fn paged(first: u64, page: u64) -> PagedSource {
    PagedSource {
        kind: "setting",
        key: "key",
        matches: |t| t == "config",
        queries: |_| vec![Query::new().eq("value", "a").asc("key"), Query::new().eq("value", "b").asc("key")],
        first,
        page,
    }
}

async fn seven(s: &Store) {
    for (k, v) in [("k1", "a"), ("k2", "b"), ("k3", "a"), ("k4", "b"), ("k5", "a"), ("k6", "b"), ("k7", "a")] {
        set(s, k, v).await;
    }
}

/// Every page `snapshot_pages` hands over, in order: (seq, ids, more, full).
async fn pages_of(snaps: &StoreSnapshots, topic: &str, since: Option<u64>) -> Vec<(u64, Vec<String>, bool, bool)> {
    let got = Arc::new(std::sync::Mutex::new(Vec::new()));
    let into = got.clone();
    let sink: PageSink = Box::new(move |r, more| {
        into.lock().unwrap().push((r.seq, r.rows.iter().map(|x| x.id.clone()).collect(), more, r.full));
    });
    snaps.snapshot_pages(&Topic::parse(topic).unwrap(), since, sink).await;
    got.lock().unwrap().clone()
}

#[tokio::test]
async fn a_paged_topic_sends_a_small_first_page_then_the_rest_every_row_once_at_one_seq() {
    let s = store();
    seven(&s).await;
    let snaps = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![paged(2, 3)]);
    let pages = pages_of(&snaps, "config", None).await;
    let ids: Vec<Vec<&str>> = pages.iter().map(|p| p.1.iter().map(String::as_str).collect()).collect();
    // the queries in order (`a` values, then `b`), a page crossing from one to the next
    assert_eq!(ids, vec![vec!["k1", "k3"], vec!["k5", "k7", "k2"], vec!["k4", "k6"]]);
    assert_eq!(pages.iter().map(|p| p.2).collect::<Vec<_>>(), [true, true, false], "more on all but the last");
    assert!(pages.iter().all(|p| p.0 == snaps.head_seq() && p.3), "every page full, at one seq");
    // one read of the topic is every page joined
    let all = snaps.snapshot(&Topic::parse("config").unwrap(), None).await;
    assert_eq!(all.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["k1", "k3", "k5", "k7", "k2", "k4", "k6"]);
    assert!(all.full);
}

/// ADR-0106 (perf audit 2026-11-19 #1): each page is its own short read, so a write can land between two pages. It is
/// never lost: the later pages read the rows as they are now (a row it moved may come twice: the client keys by id),
/// the last page carries the changes since page 1 at the seq it read to, and a resume from page 1's seq carries them too.
#[tokio::test(flavor = "multi_thread")]
async fn a_write_between_two_pages_is_in_a_later_page_or_the_patches_after_the_snapshots_seq() {
    let s = store();
    seven(&s).await;
    let snaps = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![paged(2, 3)]);
    let head = snaps.head_seq();
    let got = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (into, w) = (got.clone(), s.clone());
    let sink: PageSink = Box::new(move |r, more| {
        let first = into.lock().unwrap().is_empty();
        into.lock().unwrap().push((r.seq, r.rows.iter().map(|x| x.id.clone()).collect::<Vec<_>>(), more));
        if first {
            // the next page is read once this one is handed over: these land between the two
            w.write_blocking(|w| {
                w.upsert("setting", row!({"key": "k8", "value": "a", "locked_by": null}))?;
                w.upsert("setting", row!({"key": "k3", "value": "b", "locked_by": null}))?;
                Ok(())
            })
            .unwrap();
        }
    });
    snaps.snapshot_pages(&Topic::parse("config").unwrap(), None, sink).await;
    let pages = got.lock().unwrap().clone();
    let ids: Vec<&str> = pages.iter().flat_map(|p| p.1.iter().map(String::as_str)).collect();
    // the last page also folds in what changed since page 1 (2026-11-22 audit: a row that moved behind the keyset
    // cursor would otherwise only come as a patch for a row the client never got), and goes at the seq it read to
    assert_eq!(
        ids,
        ["k1", "k3", "k5", "k7", "k8", "k2", "k3", "k4", "k6", "k8", "k3"],
        "k8 in a later page; k3 again where it moved to; both again in the last page's delta"
    );
    let (last, rest) = pages.split_last().unwrap();
    assert!(rest.iter().all(|p| p.0 == head), "every page but the last at the first read's seq: {pages:?}");
    assert!(last.0 > head, "the last page at the seq its delta read to: {pages:?}");
    assert_eq!(pages.iter().map(|p| p.2).collect::<Vec<_>>(), [true, true, true, false]);
    let after: Vec<String> = pages_of(&snaps, "config", Some(head)).await.into_iter().flat_map(|p| p.1).collect();
    assert!(after.contains(&"k8".to_owned()) && after.contains(&"k3".to_owned()), "the patches after its seq: {after:?}");
}

#[tokio::test]
async fn rows_that_end_on_a_page_close_the_snapshot_with_an_empty_last_page_and_a_small_topic_is_one_page() {
    let s = store();
    for k in ["k1", "k2", "k3", "k4"] {
        set(&s, k, "a").await;
    }
    let snaps = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![paged(2, 2)]);
    let sizes: Vec<(usize, bool)> = pages_of(&snaps, "config", None).await.iter().map(|p| (p.1.len(), p.2)).collect();
    assert_eq!(sizes, [(2, true), (2, true), (0, false)]);
    let one = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![paged(100, 500)]);
    let sizes: Vec<(usize, bool)> = pages_of(&one, "config", None).await.iter().map(|p| (p.1.len(), p.2)).collect();
    assert_eq!(sizes, [(4, false)], "fewer rows than the first page: one page, as before");
}

#[tokio::test]
async fn a_paged_topic_resumes_from_a_cursor_as_one_delta_of_what_changed() {
    let s = store();
    seven(&s).await;
    let snaps = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![paged(2, 3)]);
    let at = snaps.head_seq();
    set(&s, "k4", "a").await;
    let pages = pages_of(&snaps, "config", Some(at)).await;
    assert_eq!(pages.len(), 1);
    assert_eq!((pages[0].1.clone(), pages[0].2, pages[0].3), (vec!["k4".to_owned()], false, false));
}

#[tokio::test]
async fn a_paged_topic_reads_every_row_after_retention_pruned_its_change_feed() {
    let clock = Arc::new(ManualClock::new(1_000));
    let s = Store::open(Location::Memory, clock.clone()).unwrap();
    seven(&s).await;
    clock.advance(8 * 24 * 60 * 60 * 1000);
    assert!(s.prune().await.unwrap().changes > 0);
    assert!(s.changed_since("config", 0).unwrap().is_none(), "the feed no longer holds the topic");
    let snaps = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![paged(2, 3)]);
    let n: usize = pages_of(&snaps, "config", None).await.iter().map(|p| p.1.len()).sum();
    assert_eq!(n, 7, "a first look is a read of the rows, never a replay of the feed");
    // a cursor retention dropped: the whole topic in pages, which the client takes in place of its rows
    let pages = pages_of(&snaps, "config", Some(1)).await;
    assert_eq!(pages.iter().map(|p| p.1.len()).sum::<usize>(), 7);
    assert!(pages.iter().all(|p| p.3));
}

#[tokio::test]
async fn a_paged_kind_not_defined_yet_reads_one_empty_page_that_closes_the_snapshot() {
    let s = store();
    set(&s, "theme", "a").await;
    let ghost = PagedSource { kind: "not_yet", key: "id", matches: |t| t == "config", queries: |_| vec![Query::new()], first: 2, page: 3 };
    let snaps = StoreSnapshots::new(s.clone(), Vec::new(), Vec::new()).with_pages(vec![ghost]);
    let pages = pages_of(&snaps, "config", None).await;
    assert_eq!(pages.iter().map(|p| (p.1.len(), p.2)).collect::<Vec<_>>(), [(0, false)]);
}
