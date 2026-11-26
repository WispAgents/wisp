//! `opencode stats` (tests/cli/stats.txt, captured from the real CLI): the boxed OVERVIEW and COST & TOKENS tables,
//! read as numbers. It's how much opencode itself says it cost and read, all time or over `--days N`.

/// What `opencode stats` reports (rounded as it prints them: `30.3M` is 30,300,000).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OpencodeStats {
    pub sessions: Option<u64>,
    pub cost_usd: Option<f64>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

/// `$0.00`, `9,540`, `328.4K`, `30.3M`, `1.2B`.
fn number(s: &str) -> Option<f64> {
    let s = s.trim().trim_start_matches('$').replace(',', "");
    let (head, mul) = match s.chars().last()? {
        'K' | 'k' => (&s[..s.len() - 1], 1e3),
        'M' => (&s[..s.len() - 1], 1e6),
        'B' => (&s[..s.len() - 1], 1e9),
        _ => (s.as_str(), 1.0),
    };
    head.parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0).map(|x| x * mul)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // finite, ≥ 0: a count
fn count(x: f64) -> u64 {
    x.round() as u64
}

/// The tables' rows by label; None when nothing in `out` is one of them (an error, another version's layout).
pub fn stats(out: &str) -> Option<OpencodeStats> {
    let mut s = OpencodeStats::default();
    let mut any = false;
    for line in out.lines() {
        let row = line.trim().trim_matches(|c: char| c == '│' || c == '|').trim();
        let Some((label, value)) = row.rsplit_once(char::is_whitespace) else { continue };
        let Some(v) = number(value) else { continue };
        let slot = match label.trim() {
            "Sessions" => &mut s.sessions,
            "Input" => &mut s.input,
            "Output" => &mut s.output,
            "Cache Read" => &mut s.cache_read,
            "Cache Write" => &mut s.cache_write,
            "Total Cost" => {
                s.cost_usd = Some(v);
                any = true;
                continue;
            }
            _ => continue,
        };
        *slot = Some(count(v));
        any = true;
    }
    any.then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_reads_the_real_cli_tables() {
        let s = stats(include_str!("../tests/cli/stats.txt"));
        assert_eq!(
            s,
            Some(OpencodeStats {
                sessions: Some(878),
                cost_usd: Some(0.0),
                input: Some(30_300_000),
                output: Some(1_700_000),
                cache_read: Some(253_500_000),
                cache_write: Some(1_800_000),
            })
        );
        assert_eq!(stats("Error: no database"), None);
    }
}
