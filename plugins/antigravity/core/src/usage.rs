//! Antigravity CLI's (`agy -p "/usage" --output-format json`) quota response → wisp's `LimitsReport` JSON.
//! Pure, sans-IO: `now_ms` comes in from the host so untouched rolling windows (whose `reset_time` slides with
//! `now + duration`) report no reset time ("New window") instead of shifting on every read.

use serde_json::{Value, json};

const MIN: i64 = 60_000;
const H: i64 = 3_600_000;
const FIVE_H: i64 = 5 * H;
const DAY: i64 = 24 * H;
const WEEK: i64 = 7 * DAY;
/// An untouched rolling window's `reset_time` is `now + duration`; allow clock / round-trip slack.
const UNUSED_SLACK_MS: i64 = 2 * MIN;

/// Map Antigravity's bucket id & window kind to wisp's canonical `(window_id, duration_ms)`.
fn bucket_spec(id: &str, window: &str) -> (String, Option<i64>) {
    match id {
        "gemini-5h" => ("five_hour".into(), Some(FIVE_H)),
        "gemini-weekly" => ("seven_day".into(), Some(WEEK)),
        "3p-5h" => ("five_hour_3p".into(), Some(FIVE_H)),
        "3p-weekly" => ("seven_day_3p".into(), Some(WEEK)),
        "gemini-argon-wtu-weekly" => ("seven_day_argon".into(), Some(WEEK)),
        other => {
            let ms = match window {
                "5h" => Some(FIVE_H),
                "weekly" => Some(WEEK),
                "daily" => Some(DAY),
                _ => None,
            };
            (other.to_owned(), ms)
        }
    }
}

fn rank(id: &str) -> usize {
    match id {
        "five_hour" => 0,
        "seven_day" => 1,
        "five_hour_3p" => 2,
        "seven_day_3p" => 3,
        "seven_day_argon" => 4,
        _ => 5,
    }
}

/// Days from 1970-01-01 (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parse an RFC3339 timestamp (`YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)`) into Unix epoch ms.
pub fn iso_ms(input: &str) -> Option<i64> {
    let bytes = input.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || (bytes[10] != b'T' && bytes[10] != b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let num = |start: usize, end: usize| input.get(start..end)?.parse::<i64>().ok();
    let (year, month, day, hour, min, sec) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    let mut pos = 19;
    let mut ms = 0_i64;
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        let start = pos;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        let frac = input.get(start..pos)?;
        if frac.is_empty() {
            return None;
        }
        let mut mul = 100_i64;
        for ch in frac.bytes().take(3) {
            ms += i64::from(ch - b'0') * mul;
            mul /= 10;
        }
    }
    let offset_sec = match bytes.get(pos) {
        Some(b'Z' | b'z') if pos + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-')) if pos + 6 == bytes.len() && bytes[pos + 3] == b':' => {
            let oh = num(pos + 1, pos + 3)?;
            let om = num(pos + 4, pos + 6)?;
            let total = oh * 3600 + om * 60;
            if *sign == b'+' { total } else { -total }
        }
        _ => return None,
    };
    let secs = days_from_civil(year, month, day) * 86_400 + hour * 3600 + min * 60 + sec - offset_sec;
    Some(secs * 1000 + ms)
}

/// Extract the JSON payload from `agy -p "/usage" --output-format json` stdout.
fn parse_json(stdout: &str) -> Option<Value> {
    let trimmed = stdout.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed)
        && v.is_object()
    {
        return Some(v);
    }
    trimmed.lines().rev().find_map(|l| serde_json::from_str::<Value>(l.trim()).ok().filter(Value::is_object))
}

/// Parse `agy -p "/usage" --output-format json` output into wisp's `LimitsReport` JSON (`report_from_value`).
pub fn usage_limits(stdout: &str, now_ms: i64) -> Option<Value> {
    let root = parse_json(stdout)?;
    if root.get("status").and_then(Value::as_str) != Some("SUCCESS") {
        return None;
    }
    let groups = root.pointer("/command/data/groups").and_then(Value::as_array)?;
    let mut windows: Vec<(String, f64, Option<i64>, Option<i64>)> = Vec::new();
    for g in groups {
        let Some(buckets) = g.get("buckets").and_then(Value::as_array) else { continue };
        for b in buckets {
            let Some(raw_id) = b.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) else { continue };
            let Some(rem) = b.get("remaining_fraction").and_then(Value::as_f64).filter(|x| x.is_finite()) else { continue };
            let win_kind = b.get("window").and_then(Value::as_str).unwrap_or_default();
            let (id, ms) = bucket_spec(raw_id, win_kind);
            if windows.iter().any(|(x, ..)| *x == id) {
                continue;
            }
            let used = (1.0 - rem).clamp(0.0, 1.0);
            let mut resets_at = b.get("reset_time").and_then(Value::as_str).and_then(iso_ms);
            // Untouched rolling windows report reset_time = now + duration on every query; drop it so wisp shows
            // "New window" instead of emitting false reset-shifted notices or closing fake 0% periods.
            if used <= 1e-9
                && let (Some(r), Some(dur)) = (resets_at, ms)
                && now_ms > 0
                && r >= now_ms + dur - UNUSED_SLACK_MS
            {
                resets_at = None;
            }
            windows.push((id, used, resets_at, ms));
        }
    }
    if windows.is_empty() {
        return None;
    }
    windows.sort_by_key(|(id, ..)| rank(id));
    let rejected = windows.iter().find(|(_, used, ..)| *used >= 1.0).map(|(id, ..)| id.clone());
    let list: Vec<Value> = windows
        .into_iter()
        .map(|(id, used, resets_at, ms)| {
            let mut w = serde_json::Map::new();
            w.insert("id".into(), json!(id));
            w.insert("used".into(), json!(used));
            if let Some(r) = resets_at {
                w.insert("resetsAt".into(), json!(r));
            }
            if let Some(m) = ms {
                w.insert("ms".into(), json!(m));
            }
            Value::Object(w)
        })
        .collect();
    let mut out = serde_json::Map::new();
    out.insert("windows".into(), Value::Array(list));
    out.insert("complete".into(), json!(true));
    if let Some(rej) = rejected {
        out.insert("rejected".into(), json!(rej));
    }
    Some(Value::Object(out))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"conversation_id":"","status":"SUCCESS","response":"Gemini Models\tWeekly Limit Remaining\t40%\t2026-11-28T11:31:09Z\n","duration_seconds":0,"num_turns":0,"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0},"command":{"name":"usage","data":{"groups":[{"name":"Gemini Models","buckets":[{"id":"gemini-weekly","name":"Weekly Limit Remaining","window":"weekly","remaining_fraction":0.4,"reset_time":"2026-11-28T11:31:09Z"},{"id":"gemini-5h","name":"Five Hour Limit Remaining","window":"5h","remaining_fraction":0.77,"reset_time":"2026-11-23T03:30:26Z"}]},{"name":"Claude and GPT models","buckets":[{"id":"3p-weekly","name":"Weekly Limit Remaining","window":"weekly","remaining_fraction":1,"reset_time":"2026-11-29T22:41:21Z"},{"id":"3p-5h","name":"Five Hour Limit Remaining","window":"5h","remaining_fraction":1,"reset_time":"2026-11-23T03:41:21Z"}]},{"name":"Gemini 4 Argon","buckets":[{"id":"gemini-argon-wtu-weekly","name":"Gemini 4 Argon Weekly Limit","window":"weekly","remaining_fraction":1,"reset_time":"2026-11-29T22:41:21Z"}]}]}}}"#;

    #[test]
    fn parses_real_agy_usage_json_and_drops_sliding_resets_for_unused_buckets() {
        let now = iso_ms("2026-11-22T22:41:21Z").unwrap();
        let report = usage_limits(SAMPLE, now).unwrap();
        assert_eq!(report["complete"], json!(true));
        assert!(report.get("rejected").is_none());
        let ws = report["windows"].as_array().unwrap();
        let ids: Vec<&str> = ws.iter().map(|w| w["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["five_hour", "seven_day", "five_hour_3p", "seven_day_3p", "seven_day_argon"]);
        assert!((ws[0]["used"].as_f64().unwrap() - 0.23).abs() < 1e-9);
        assert_eq!(ws[0]["resetsAt"], json!(iso_ms("2026-11-23T03:30:26Z").unwrap()));
        assert_eq!(ws[0]["ms"], json!(FIVE_H));
        assert!((ws[1]["used"].as_f64().unwrap() - 0.60).abs() < 1e-9);
        assert_eq!(ws[1]["resetsAt"], json!(iso_ms("2026-11-28T11:31:09Z").unwrap()));
        assert_eq!(ws[1]["ms"], json!(WEEK));
        // Untouched buckets whose reset_time == now + duration have no resetsAt ("New window")
        for w in &ws[2..] {
            assert_eq!(w["used"], json!(0.0));
            assert!(w.get("resetsAt").is_none(), "sliding reset dropped: {w}");
        }
    }

    #[test]
    fn marks_full_bucket_as_rejected_and_ignores_errors() {
        let now = iso_ms("2026-11-22T22:41:21Z").unwrap();
        let full = r#"{"status":"SUCCESS","command":{"name":"usage","data":{"groups":[{"name":"Gemini Models","buckets":[{"id":"gemini-5h","window":"5h","remaining_fraction":0,"reset_time":"2026-11-23T03:30:26Z"}]}]}}}"#;
        let report = usage_limits(full, now).unwrap();
        assert_eq!(report["rejected"], json!("five_hour"));
        assert_eq!(report["windows"][0]["used"], json!(1.0));
        assert!(usage_limits(r#"{"status":"ERROR","error":"API key has no quota"}"#, now).is_none());
        assert!(usage_limits("not json", now).is_none());
    }

    #[test]
    fn iso_timestamps_parse_utc_fractional_and_offsets() {
        assert_eq!(iso_ms("1970-01-01T00:00:01.500Z"), Some(1500));
        assert_eq!(iso_ms("2026-11-08T10:00:00Z"), Some(1_794_132_000_000));
        assert_eq!(iso_ms("2026-11-08T11:00:00+01:00"), Some(1_794_132_000_000));
        assert_eq!(iso_ms("nope"), None);
    }
}
