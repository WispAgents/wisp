//! The note before a follow-up and the supersession nudge (v1 test/threads.test.js `ledgerNote`, `supersedes`), with
//! generated messages: a seeded loop over every phrase in every position and case, and over messages that hold none.

use wisp_threads_core::note::{NoteIn, lead_note, supersedes, turn_note};

/// A small seeded generator (xorshift), so a failure names its seed.
struct Gen(u64);
impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[usize::try_from(self.next() % xs.len() as u64).unwrap_or(0)]
    }
}

const FILLER: [&str; 12] = [
    "add a test for login",
    "now do the docs",
    "is it fast?",
    "know your users",
    "the build",
    "please",
    "run it again",
    "and the README",
    "with tabs",
    "keep going",
    "looks good",
    "thanks",
];
const TAKE_BACK: [&str; 10] = [
    "Actually",
    "instead",
    "scratch that",
    "never mind",
    "no longer",
    "anymore",
    "rather than",
    "switch to",
    "let’s not",
    "on second thought",
];

fn case(g: &mut Gen, s: &str) -> String {
    match g.next() % 3 {
        0 => s.to_uppercase(),
        1 => s.to_lowercase(),
        _ => s.to_owned(),
    }
}

#[test]
fn any_take_back_phrase_anywhere_in_any_case_is_a_supersession() {
    for seed in 1..400u64 {
        let mut g = Gen(seed * 2_654_435_761);
        let p = g.pick(&TAKE_BACK);
        let phrase = case(&mut g, p);
        let msg = format!("{} {phrase}, {}", g.pick(&FILLER), g.pick(&FILLER));
        assert!(supersedes(&msg), "seed {seed}: {msg}");
    }
}

#[test]
fn messages_made_of_ordinary_words_are_not() {
    for seed in 1..400u64 {
        let mut g = Gen(seed * 40_503);
        let mut msg: Vec<String> = Vec::new();
        for _ in 0..=(g.next() % 4) {
            let p = g.pick(&FILLER);
            msg.push(case(&mut g, p));
        }
        let msg = msg.join(". ");
        assert!(!supersedes(&msg), "seed {seed}: {msg}");
    }
    for t in ["Nowhere to go", "Notable change", "actuallyness", "waiter", "Stopwatch app", "Insteadof is a word here"] {
        assert!(!supersedes(t), "{t}: a phrase inside a word isn't one");
    }
    for t in ["No, keep the old API", "Wait: not that file", "Stop. Revert that", "we no longer need the cache"] {
        assert!(supersedes(t), "{t}");
    }
}

#[test]
fn the_note_carries_the_notes_what_you_changed_and_the_nudge() {
    let n = NoteIn {
        notes: "## Goal\nG\n\n## Constraints\n- C1",
        rev: 3,
        yours_unseen: true,
        turn: 3,
        prompt: "Carry on",
        tool: "mcp__wisp__wisp_notes",
    };
    let s = turn_note(&n).unwrap();
    assert!(s.starts_with("<task-notes rev=\"3\">") && s.contains("## Constraints\n- C1") && s.contains("The user edited them"));
    assert!(!s.contains("may change something said earlier"));
    let seen = turn_note(&NoteIn { yours_unseen: false, ..n }).unwrap();
    assert!(!seen.contains("user edited"), "seen: not said again");
    let back = turn_note(&NoteIn { prompt: "Actually, use Postgres instead", ..n }).unwrap();
    assert!(back.contains("rewrite that section of the Notes with `mcp__wisp__wisp_notes`"));
    // no Notes: a nudge now and then, from the third turn on; nothing on the first turn
    let none = NoteIn { notes: "", ..n };
    assert!(turn_note(&NoteIn { turn: 2, ..none }).unwrap().contains("no Notes yet"));
    assert_eq!(turn_note(&NoteIn { turn: 3, ..none }), None);
    assert_eq!(turn_note(&NoteIn { turn: 0, prompt: "actually do it", ..none }), None, "the first turn takes nothing back");
    assert!(lead_note("## Goal\nG").unwrap().starts_with("<lead-notes>"));
    assert_eq!(lead_note("  "), None);
}
