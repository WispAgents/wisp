//! The compaction audit's scorer (ADR-0096): an item the answer repeats, in any order and case, is kept; one it never
//! mentions is lost; an empty answer loses every item with words; the items are the audited sections' only.
#![allow(clippy::unwrap_used)]

use wisp_threads_core::audit::{ITEMS_MAX, RECALL, items, kept, lost_note, score, words};
use wisp_threads_core::notes::Shown;

/// A seeded xorshift: the same cases every run.
struct Gen(u64);
impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[usize::try_from(self.next() % xs.len() as u64).unwrap()]
    }
}

const VOCAB: [&str; 16] = [
    "sqlite", "redis", "postgres", "importer", "parser", "keep", "public", "api", "retry", "timeout", "worker", "queue", "csv", "json",
    "schema", "branch",
];
const OTHER: [&str; 8] = ["banana", "violin", "glacier", "orbit", "pepper", "lantern", "meadow", "copper"];

fn item(g: &mut Gen) -> String {
    let n = 2 + g.next() % 4;
    (0..n).map(|_| g.pick(&VOCAB)).collect::<Vec<_>>().join(" ")
}

#[test]
fn an_item_the_answer_repeats_in_any_order_and_case_is_kept() {
    let mut g = Gen(0x5eed_a0d1);
    for _ in 0..500 {
        let it = item(&mut g);
        let mut ws: Vec<String> = it.split(' ').map(str::to_uppercase).collect();
        ws.reverse();
        let recall = format!("- I remember: {}. Also {}.", ws.join(", "), g.pick(&OTHER));
        assert!(kept(&format!("Constraints: {it}"), &recall), "{it} / {recall}");
    }
}

#[test]
fn an_item_the_answer_never_mentions_is_lost_and_an_empty_answer_loses_all() {
    let mut g = Gen(0xbad_c0de);
    for _ in 0..500 {
        let it = item(&mut g);
        let recall = (0..6).map(|_| g.pick(&OTHER)).collect::<Vec<_>>().join(" ");
        assert!(!kept(&format!("Decisions: {it}"), &recall), "{it} / {recall}");
        assert!(!kept(&format!("Decisions: {it}"), ""));
    }
    // an item with no content words can't be checked: kept
    assert!(kept("Goal: do it", ""));
    let all = vec!["Goal: Import CSV files".to_owned(), "Constraints: No Redis".to_owned(), "Failed: Postgres timed out".to_owned()];
    let (k, lost) = score(&all, "The goal is to import the csv files. Don't know the rest.");
    assert_eq!((k, lost), (1, vec!["Constraints: No Redis".to_owned(), "Failed: Postgres timed out".to_owned()]));
}

#[test]
fn the_items_are_the_audited_sections_one_a_line_and_capped() {
    let many = (0..30).map(|i| format!("decision {i}")).collect::<Vec<_>>().join("\n");
    let sections = [
        Shown { id: "goal", text: "Ship the importer" },
        Shown { id: "state", text: "Half done" },
        Shown { id: "constraints", text: "No Redis\n\nKeep the API" },
        Shown { id: "decisions", text: &many },
    ];
    let it = items(&sections);
    assert_eq!(&it[..3], &["Goal: Ship the importer", "Constraints: No Redis", "Constraints: Keep the API"]);
    assert_eq!(it.len(), ITEMS_MAX);
    assert!(it.iter().all(|i| !i.starts_with("Where it stands")), "state isn't audited");
    assert_eq!(words("The Workers' queues, and the API's timeout"), vec!["worker", "queue", "api", "timeout"]);
    assert!(RECALL.contains("Do not use tools"));
    let note = lost_note(&["Constraints: No Redis".into()], "mcp__wisp__wisp_notes").unwrap();
    assert!(note.contains("<compaction-lost>") && note.contains("- Constraints: No Redis") && note.contains("mcp__wisp__wisp_notes"));
    assert_eq!(lost_note(&[], "x"), None);
}
