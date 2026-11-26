//! Whose model a harness runs (Anthropic, `OpenAI`, Google, `xAI`…), for review by another vendor (ADR-0132). The same
//! table as the refusal card's (`plugins/shell-task/ui/src/cards/vendors.ts`, v1 web/js/vendors.js); the golden
//! `tests/goldens/vendors.json` keeps the two in step. A model id names its vendor; a harness without one is sure only
//! when it runs one vendor's models.

/// (vendor, the words in a model id that name it), first match wins.
const VENDORS: &[(&str, &[&str])] = &[
    ("anthropic", &["claude", "anthropic", "opus", "sonnet", "haiku", "fable"]),
    ("openai", &["openai", "chatgpt", "gpt", "codex"]),
    ("google", &["gemini", "gemma", "google", "vertex"]),
    ("xai", &["grok", "xai", "x-ai"]),
    ("deepseek", &["deepseek"]),
    ("alibaba", &["qwen"]),
    ("moonshot", &["kimi", "moonshot"]),
    ("zhipu", &["glm", "zhipu", "z-ai"]),
    ("mistral", &["mistral", "codestral", "devstral"]),
    ("meta", &["llama"]),
    ("cursor", &["composer"]),
];

/// What a harness runs when it isn't given a model: only the single-vendor ones are sure.
const HOME: &[(&str, &str)] = &[("claude", "anthropic"), ("codex", "openai"), ("grok", "xai")];

/// `OpenAI`'s o-series ids (`o3`, `o4-mini`, `openai/o1`): an `o` and digits as a whole part.
fn o_series(m: &str) -> bool {
    m.split(['/', ':', ' ', '_', '-', '.']).any(|p| p.len() >= 2 && p.starts_with('o') && p[1..].chars().all(|c| c.is_ascii_digit()))
}

/// The vendor of `model` on `agent` ("" when nobody can tell).
pub fn vendor_of(model: &str, agent: &str) -> &'static str {
    let m = model.trim().to_ascii_lowercase();
    if !m.is_empty() && m != "auto" && m != "default" {
        for (v, words) in VENDORS {
            if words.iter().any(|w| m.contains(w)) {
                return v;
            }
        }
        if o_series(&m) {
            return "openai";
        }
    }
    HOME.iter().find(|(a, _)| *a == agent).map_or("", |(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::vendor_of;

    /// The golden both tables read: every id there names the vendor next to it, on the harness next to it.
    #[test]
    fn the_vendor_table_matches_the_golden_the_ui_reads_too() {
        let raw = include_str!("../tests/goldens/vendors.json");
        let cases: Vec<(String, String, String)> = serde_json::from_str(raw).unwrap_or_default();
        assert!(cases.len() > 10, "the golden has its cases");
        for (model, agent, want) in cases {
            assert_eq!(vendor_of(&model, &agent), want, "{model} on {agent}");
        }
    }
}
