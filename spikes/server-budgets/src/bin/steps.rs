//! What real agent turns keep, once wisp has parsed them: the evidence behind the stream the write-amplification gate
//! runs (`FAKE_STREAM_MIX`, README *The stream's shape*).
//!
//! Reads Claude Code transcripts (`<dir>/*.jsonl`, the CLI's own session logs: one line per content block, as its
//! `stream-json` prints them, each with its timestamp), feeds each main-thread line to wisp's own Claude parser
//! (`wisp-claude-core`) and each event to the runtime's `activity::apply`, the path `drive.rs` takes, and measures the
//! JSON of every step a turn would keep (the gate's payload). Prints sizes and counts only, never a line's text.
//!
//! ```sh
//! cargo run --release --manifest-path spikes/server-budgets/Cargo.toml --bin steps -- <dir>… [--files=<n>]
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use wisp_claude_core::{ClaudeOptions, ClaudeParser};
use wisp_harness_kit::{LineParser, Out, RunPaths};
use wisp_runtime_core::activity::{self, Effect, Head};

/// ISO 8601 UTC (`2026-11-15T12:34:56.789Z`) → unix ms; anything else is `None`.
fn iso_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r).and_then(|x| x.parse::<i64>().ok());
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    let ms = if b.get(19) == Some(&b'.') { n(20..23).unwrap_or(0) } else { 0 };
    // days from civil (Howard Hinnant)
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + h) * 60 + mi) * 60_000 + se * 1000 + ms)
}

/// A user line that starts a turn: a prompt (text), not a tool's result.
fn is_prompt(v: &Value) -> bool {
    if v["isMeta"].as_bool() == Some(true) {
        return false;
    }
    match &v["message"]["content"] {
        Value::String(_) => true,
        Value::Array(parts) => !parts.iter().any(|p| p["type"] == "tool_result"),
        _ => false,
    }
}

#[derive(Default)]
struct Turn {
    /// (ms since the prompt, kind, bytes)
    steps: Vec<(i64, &'static str, usize)>,
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    let i = ((p / 100.0) * (v.len() - 1) as f64).round() as usize;
    v[i]
}

fn line(name: &str, v: &mut [f64]) {
    #[allow(clippy::cast_precision_loss)]
    let mean = if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    println!(
        "{name:<34} n {:>7}  mean {:>8.1}  p50 {:>8.1}  p90 {:>8.1}  p99 {:>8.1}  max {:>9.1}",
        v.len(),
        mean,
        pct(v, 50.0),
        pct(v, 90.0),
        pct(v, 99.0),
        pct(v, 100.0)
    );
}

fn turns_of(text: &str) -> Vec<Turn> {
    let mut p = ClaudeParser::new(ClaudeOptions { paths: RunPaths::same("/tmp/w"), ..ClaudeOptions::default() });
    let mut head = Head::default();
    let mut out: Vec<Out> = Vec::new();
    let mut turns: Vec<Turn> = Vec::new();
    let mut start: Option<i64> = None;
    for raw in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(raw) else { continue };
        let ty = v["type"].as_str().unwrap_or_default();
        if !matches!(ty, "user" | "assistant") || v["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        let Some(at) = v["timestamp"].as_str().and_then(iso_ms) else { continue };
        if ty == "user" && is_prompt(&v) {
            turns.push(Turn::default());
            start = Some(at);
            head = Head::default();
            continue;
        }
        let (Some(t0), Some(turn)) = (start, turns.last_mut()) else { continue };
        p.line(raw, 0, &mut out);
        for o in out.drain(..) {
            let Out::Event(ev) = o else { continue };
            // the kit's event as the runtime's (wisp.claude's `to_runtime`, without its stats clean-up: steps carry none)
            let Ok(ev) = wisp_runtime_api::HarnessEvent::from_v1(ev.to_json()) else { continue };
            if let Effect::Append(a) | Effect::Waiting(a) = activity::apply(&mut head, &ev, false, &|_| false) {
                let bytes = serde_json::to_vec(&a).map_or(0, |b| b.len());
                turn.steps.push((at - t0, a.kind(), bytes));
            }
        }
    }
    turns
}

#[allow(clippy::cast_precision_loss)]
fn main() {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut max_files = usize::MAX;
    for a in std::env::args().skip(1) {
        if let Some(n) = a.strip_prefix("--files=") {
            max_files = n.parse().unwrap_or(usize::MAX);
        } else {
            dirs.push(a.into());
        }
    }
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    files.truncate(max_files);

    let mut sizes: Vec<f64> = Vec::new();
    let mut by_kind: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    let (mut turn_rate, mut turn_steps, mut turn_secs, mut turn_bps) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut gaps, mut win_steps, mut win_bytes) = (Vec::new(), Vec::new(), Vec::new());
    let (mut windows_all, mut windows_busy) = (0u64, 0u64);
    // a turn's busiest 10 s (its peak rate) and every 1 s it was at work (a step in it)
    let (mut peak10, mut busy_sec) = (Vec::new(), Vec::new());
    let mut n_turns = 0usize;
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for t in turns_of(&text) {
            if t.steps.is_empty() {
                continue;
            }
            n_turns += 1;
            for (_, k, b) in &t.steps {
                sizes.push(*b as f64);
                by_kind.entry(k).or_default().push(*b as f64);
            }
            for w in t.steps.windows(2) {
                gaps.push((w[1].0 - w[0].0) as f64);
            }
            // the turn's own life: its prompt to its last step (a turn of one step at 0 ms is no rate)
            let last = t.steps.last().map_or(0, |s| s.0);
            let total: usize = t.steps.iter().map(|s| s.2).sum();
            if last >= 1000 {
                let secs = last as f64 / 1000.0;
                turn_rate.push(t.steps.len() as f64 / secs);
                turn_bps.push(total as f64 / secs);
                turn_secs.push(secs);
            }
            turn_steps.push(t.steps.len() as f64);
            let mut secs: BTreeMap<i64, usize> = BTreeMap::new();
            for (ms, _, _) in &t.steps {
                *secs.entry(ms.div_euclid(1000)).or_default() += 1;
            }
            busy_sec.extend(secs.values().map(|n| *n as f64));
            let peak = secs.keys().map(|s0| secs.range(*s0..s0 + 10).map(|(_, n)| n).sum::<usize>()).max().unwrap_or(0);
            if last >= 10_000 {
                peak10.push(peak as f64 / 10.0);
            }
            // what one task hands each 100 ms commit window (ADR-0083) while its turn runs
            let mut win: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
            for (ms, _, b) in &t.steps {
                let e = win.entry(ms.div_euclid(100)).or_default();
                e.0 += 1;
                e.1 += b;
            }
            windows_all += u64::try_from(last / 100 + 1).unwrap_or(0);
            windows_busy += win.len() as u64;
            for (n, b) in win.values() {
                win_steps.push(*n as f64);
                win_bytes.push(*b as f64);
            }
        }
    }
    println!("{} transcripts, {n_turns} turns with steps, {} steps", files.len(), sizes.len());
    println!("\n-- bytes per kept step (the activity's JSON, the gate's payload) --");
    line("all", &mut sizes);
    for (k, v) in &mut by_kind {
        line(k, v);
    }
    let total = sizes.len().max(1) as f64;
    let mix: Vec<String> = by_kind.iter().map(|(k, v)| format!("{k} {:.1}%", 100.0 * v.len() as f64 / total)).collect();
    println!("mix: {}", mix.join(", "));
    let mut all = sizes.clone();
    let q: Vec<String> = (0..20).map(|i| format!("{:.0}", pct(&mut all, 2.5 + 5.0 * f64::from(i)))).collect();
    println!("the 20 ventiles' midpoints (2.5%, 7.5%, … 97.5%): {}", q.join(" "));
    println!("\n-- per turn --");
    line("steps", &mut turn_steps);
    line("seconds (prompt to last step, ≥1 s)", &mut turn_secs);
    line("steps / s", &mut turn_rate);
    line("kept bytes / s", &mut turn_bps);
    line("ms between steps", &mut gaps);
    line("steps / s, the busiest 10 s (≥10 s)", &mut peak10);
    line("steps in a 1 s with a step", &mut busy_sec);
    println!("\n-- per task, per 100 ms window (ADR-0083) --");
    println!("windows with a step: {windows_busy} of {windows_all} ({:.1}%)", 100.0 * windows_busy as f64 / windows_all.max(1) as f64);
    line("steps in a busy window", &mut win_steps);
    line("bytes in a busy window", &mut win_bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_iso_stamp_is_unix_ms_and_garbage_is_none() {
        assert_eq!(iso_ms("1970-01-01T00:00:01.250Z"), Some(1250));
        assert_eq!(iso_ms("2026-11-16T00:00:00Z"), Some(1_794_787_200_000));
        assert_eq!(iso_ms("nope"), None);
    }
}
