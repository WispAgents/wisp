//! A [`Scrub`] made ready once (perf audit 2026-11-18 #7): every other task's and crew member's id as one matcher
//! (a regex alternation of literals, which the regex crate runs as Aho-Corasick), the folder forms worked out once,
//! and the mention chips' ids in a set. Each string of a bundle was searched for each of ~5,000 ids, a `String`
//! rebuilt per id: a full preview of a 2 MB task took 109–137 ms. Each thread keeps the last one it prepared (a
//! preview scrubs its title, spec, formula, report and history with the same `Scrub`, back to back).
use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use regex::Regex;

use crate::{Scrub, folder_forms, mention};

/// One kind of id to take out (`wisp` tasks, `crew` members): the set (mention chips) and the matcher (bare ids).
pub(crate) struct Ids {
    pub kind: &'static str,
    pub to: &'static str,
    set: HashSet<String>,
    bare: Option<Regex>,
}

pub(crate) struct Prepared {
    pub forms: Vec<(String, &'static str)>,
    pub ids: [Ids; 2],
}

fn word(ch: Option<char>) -> bool {
    ch.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

impl Ids {
    fn new(kind: &'static str, to: &'static str, ids: &[String]) -> Self {
        let set: HashSet<String> = ids.iter().cloned().collect();
        // ids shorter than 8 never go bare (they'd match ordinary words); longest first, so one inside another loses
        let mut bare: Vec<&String> = ids.iter().filter(|i| i.len() >= 8).collect();
        bare.sort_by_key(|i| std::cmp::Reverse(i.len()));
        bare.dedup();
        let bare =
            (!bare.is_empty()).then(|| Regex::new(&bare.iter().map(|i| regex::escape(i)).collect::<Vec<_>>().join("|")).ok()).flatten();
        Self { kind, to, set, bare }
    }

    /// `t` with each mention chip of one of these ids and each whole occurrence of one (not inside a longer word) as
    /// `to`; what went, by its title or the id, in `went`.
    pub fn replace(&self, t: &str, went: &mut Vec<String>) -> String {
        if self.set.is_empty() {
            return t.to_owned();
        }
        let chips = match mention() {
            Some(m) if t.contains("@[") => m
                .replace_all(t, |c: &regex::Captures<'_>| {
                    if &c[2] == self.kind && self.set.contains(&c[3]) {
                        went.push(c[1].to_owned());
                        self.to.to_owned()
                    } else {
                        c[0].to_owned()
                    }
                })
                .into_owned(),
            _ => t.to_owned(),
        };
        let Some(bare) = &self.bare else { return chips };
        let mut out = String::with_capacity(chips.len());
        let mut last = 0;
        for m in bare.find_iter(&chips) {
            let (before, after) = (chips[..m.start()].chars().next_back(), chips[m.end()..].chars().next());
            out.push_str(&chips[last..m.start()]);
            if word(before) || word(after) {
                out.push_str(m.as_str());
            } else {
                out.push_str(self.to);
                went.push(m.as_str().to_owned());
            }
            last = m.end();
        }
        out.push_str(&chips[last..]);
        out
    }
}

fn key(s: &Scrub) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&s.env, &s.project, &s.home, &s.tasks, &s.crew).hash(&mut h);
    h.finish()
}

impl Prepared {
    fn new(s: &Scrub) -> Self {
        Self { forms: folder_forms(&s.project, &s.home), ids: [Ids::new("wisp", "<task>", &s.tasks), Ids::new("crew", "<crew>", &s.crew)] }
    }

    /// `s` made ready, or the one this thread made for the same `Scrub` last (a preview's scrubs run back to back).
    pub fn of(s: &Scrub) -> Arc<Self> {
        thread_local! {
            static LAST: RefCell<Option<(u64, Arc<Prepared>)>> = const { RefCell::new(None) };
        }
        let k = key(s);
        LAST.with(|last| {
            if let Some((at, p)) = last.borrow().as_ref()
                && *at == k
            {
                return p.clone();
            }
            let p = Arc::new(Self::new(s));
            *last.borrow_mut() = Some((k, p.clone()));
            p
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::many_single_char_names, clippy::format_push_string)]
mod tests {
    use super::*;

    /// The scrub before perf fix #7, id by id: the reference the single matcher must agree with.
    fn reference(t: &str, kind: &str, ids: &[String], to: &str, went: &mut Vec<String>) -> String {
        let chips = mention().map_or_else(
            || t.to_owned(),
            |m| {
                m.replace_all(t, |c: &regex::Captures<'_>| {
                    if &c[2] == kind && ids.iter().any(|i| *i == c[3]) {
                        went.push(c[1].to_owned());
                        to.to_owned()
                    } else {
                        c[0].to_owned()
                    }
                })
                .into_owned()
            },
        );
        let mut out = chips;
        for id in ids.iter().filter(|i| i.len() >= 8) {
            let mut next = String::with_capacity(out.len());
            let mut rest = out.as_str();
            while let Some(at) = rest.find(id.as_str()) {
                let (before, after) = (rest[..at].chars().next_back(), rest[at + id.len()..].chars().next());
                next.push_str(&rest[..at]);
                if word(before) || word(after) {
                    next.push_str(id);
                } else {
                    next.push_str(to);
                    went.push(id.clone());
                }
                rest = &rest[at + id.len()..];
            }
            next.push_str(rest);
            out = next;
        }
        out
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn pick<'a>(&mut self, xs: &'a [String]) -> &'a str {
            &xs[usize::try_from(self.next() % xs.len() as u64).unwrap()]
        }
    }

    #[test]
    fn the_single_matcher_takes_out_exactly_what_the_id_by_id_scrub_did() {
        let mut r = Rng(0x9e37_79b9_7f4a_7c15);
        let ids: Vec<String> = (0..120).map(|i| format!("01aa{i:04}{:024x}", i * 7919)).chain(["short".to_owned()]).collect();
        let glue = ["", " ", "x", "-", "_", ".", "(", ")", "\n", "é", "/"];
        for _ in 0..120 {
            let mut t = String::new();
            for _ in 0..12 {
                match r.next() % 4 {
                    0 => t.push_str(r.pick(&ids)),
                    1 => t.push_str(&format!("@[Fix login](wisp:{})", r.pick(&ids))),
                    2 => t.push_str(&format!("@[Ada](crew:{})", r.pick(&ids))),
                    _ => t.push_str("some words"),
                }
                t.push_str(glue[usize::try_from(r.next() % glue.len() as u64).unwrap()]);
            }
            let p = Ids::new("wisp", "<task>", &ids);
            let (mut a, mut b) = (Vec::new(), Vec::new());
            assert_eq!(p.replace(&t, &mut a), reference(&t, "wisp", &ids, "<task>", &mut b), "{t}");
            // the same redactions; listed in the text's order now, where the old scrub went id by id
            a.sort();
            b.sort();
            assert_eq!(a, b, "{t}");
        }
    }

    #[test]
    fn the_last_prepared_scrub_is_reused_and_a_changed_one_isnt() {
        let s = Scrub { tasks: vec!["01aa0000payroll0000000000000001".into()], ..Scrub::default() };
        let (a, b) = (Prepared::of(&s), Prepared::of(&s));
        assert!(Arc::ptr_eq(&a, &b));
        let other = Scrub { tasks: vec!["01bb0000other00000000000000002".into()], ..s };
        assert!(!Arc::ptr_eq(&a, &Prepared::of(&other)));
    }
}
