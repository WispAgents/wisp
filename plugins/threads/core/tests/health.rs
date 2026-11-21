//! A thread's health (v1 test/threads.test.js `health`, `turnErrors`): ok, heavy, worn; a fresh thread starts the
//! count again; the same error in different words of numbers and paths is one; an edit undone counts.

use wisp_threads_api::Level;
use wisp_threads_core::health::{EditIn, TurnIn, error_sig, health};

fn t() -> TurnIn {
    TurnIn::default()
}

#[test]
fn ok_then_heavy_then_worn_from_context_compactions_turns_and_the_same_error_again() {
    let ok = health("t1", &[TurnIn { context: Some((10_000, 200_000)), ..t() }], None);
    assert_eq!((ok.level, ok.reasons.len()), (Level::Ok, 0));
    let heavy = health("t1", &[TurnIn { context: Some((130_000, 200_000)), ..t() }], None);
    assert_eq!((heavy.level, heavy.reasons.clone()), (Level::Heavy, vec!["context 65% full".to_owned()]));
    let err = vec!["Bash failed: Error at /src/a.js:12".to_owned(), "Bash failed: Error at /src/b.js:40".to_owned()];
    let worn = health(
        "t1",
        &[
            TurnIn { compact: true, ..t() },
            TurnIn { compact: true, ..t() },
            TurnIn { errors: err.clone(), ..t() },
            TurnIn { errors: err, context: Some((170_000, 200_000)), ..t() },
        ],
        None,
    );
    assert_eq!((worn.level, worn.compactions, worn.turns), (Level::Worn, 2, 2));
    assert_eq!(worn.repeat.as_ref().map(|r| r.n), Some(4), "{worn:?}");
    assert!(worn.reasons.iter().any(|r| r == "the same error 4× in the last 2 turns"), "{:?}", worn.reasons);
}

#[test]
fn a_fresh_thread_starts_the_count_again_and_old_notes_say_so() {
    let mut turns = vec![TurnIn { compact: true, ..t() }, TurnIn { compact: true, ..t() }, TurnIn { fresh: true, ..t() }];
    turns.extend((0..9).map(|_| t()));
    let h = health("t1", &turns, Some(11));
    assert_eq!((h.compactions, h.turns), (0, 10));
    assert_eq!(h.notes_age, Some(11));
    assert!(h.reasons.iter().any(|r| r == "Notes not updated in 11 turns"), "{:?}", h.reasons);
    assert_eq!(health("t1", &turns, None).notes_age, None);
}

#[test]
fn edits_the_agent_undid_and_three_failed_turns_count() {
    let e = |b: &str, a: &str| EditIn { file: "src/a.rs".into(), before: b.into(), after: a.into() };
    let turns = [TurnIn { edits: vec![e("x", "y"), e("p", "q")], ..t() }, TurnIn { edits: vec![e("y", "x"), e("q", "p")], ..t() }];
    let h = health("t1", &turns, None);
    assert_eq!((h.reverted, h.level), (2, Level::Heavy));
    let failed = health("t1", &[TurnIn { failed: true, ..t() }, TurnIn { failed: true, ..t() }, TurnIn { failed: true, ..t() }], None);
    assert!(failed.reasons.iter().any(|r| r.contains("didn’t get there")));
}

#[test]
fn an_errors_words_without_numbers_paths_or_hashes_are_its_signature() {
    assert_eq!(error_sig("Bash failed: Error: ENOENT /tmp/a/1.txt"), error_sig("Bash failed: Error: ENOENT /tmp/b/2.txt"));
    assert_eq!(error_sig("exit 127 after 3000ms"), "exit 0 after 0ms");
    assert_eq!(error_sig("commit 4f9a2c81d0 not found"), "commit # not found");
    assert_ne!(error_sig("Edit failed: no match"), error_sig("Bash failed: no match"));
}

/// ADR-0096: the harness compacting on its own inside a turn counts as a compaction, as a `/compact` turn does.
#[test]
fn compactions_inside_a_turn_count_as_compact_turns_do() {
    let h = health("t1", &[t(), TurnIn { compacted: 2, ..t() }, TurnIn { compact: true, ..t() }, t()], None);
    assert_eq!(h.compactions, 3);
    assert_eq!(h.level, Level::Heavy, "{h:?}");
    let after = health("t1", &[TurnIn { compacted: 3, ..t() }, TurnIn { fresh: true, ..t() }], None);
    assert_eq!(after.compactions, 0, "a fresh thread starts the count again");
}
