//! A secret written into a formula (AAC-4): a plaintext token in any spec string, or `${env:NAME}` in a field that
//! holds secrets ([`KindInfo::secret_fields`]), would sit in the store's row and in every export. The plan names each
//! one with its fix (`wisp secret set` and `${secret:NAME}`), and what wisp prints of specs masks what the scanner hits.

use serde_json::Value;
use wisp_envmask::secret_scan;
use wisp_resources_api::{KindInfo, Resource};

/// The secret's name the fix suggests: the usual one for a known token, else the field's own (`X_API_KEY`).
fn suggest(what: &str, key: &str) -> String {
    let known = match what {
        "a GitHub token" => "GH_TOKEN",
        "an Anthropic API key" => "ANTHROPIC_API_KEY",
        "an OpenAI API key" => "OPENAI_API_KEY",
        "an AWS access key" => "AWS_ACCESS_KEY_ID",
        "a Google API key" => "GOOGLE_API_KEY",
        "a Slack token" => "SLACK_TOKEN",
        "a Discord bot token" => "DISCORD_TOKEN",
        "a Stripe key" => "STRIPE_KEY",
        "an npm token" => "NPM_TOKEN",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    let field: String = key.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    let field = field.trim_matches('_');
    if field.is_empty() || field.starts_with(|c: char| c.is_ascii_digit()) { "TOKEN".to_owned() } else { field.to_owned() }
}

/// The `${env:NAME}` names in `s`.
fn env_refs(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(at) = rest.find("${env:") {
        let after = &rest[at + 6..];
        let Some(end) = after.find('}') else { break };
        out.push(&after[..end]);
        rest = &after[end + 1..];
    }
    out
}

/// What [`secret_scan::mask_secrets`] leaves in place of a token (`ghp_…(masked)`).
const MASK: &str = "…(masked)";

/// The secret's name the fix suggests for a masked value, from the head the mask kept (`ghp_` → `GH_TOKEN`).
fn suggest_masked(s: &str, key: &str) -> String {
    let head = s.split(MASK).next().unwrap_or_default();
    let head = head.rsplit(|c: char| c.is_whitespace() || matches!(c, '=' | ':' | '"' | '\'')).next().unwrap_or_default();
    let what = match head {
        h if h.starts_with("gh") || h.starts_with("gith") => "a GitHub token",
        h if h.starts_with("sk-a") => "an Anthropic API key",
        h if h.starts_with("sk-") => "an OpenAI API key",
        h if h.starts_with("AKIA") || h.starts_with("ASIA") => "an AWS access key",
        h if h.starts_with("AIza") => "a Google API key",
        h if h.starts_with("xox") => "a Slack token",
        h if h.starts_with("sk_l") || h.starts_with("rk_l") => "a Stripe key",
        h if h.starts_with("npm_") => "an npm token",
        _ => "",
    };
    suggest(what, key)
}

/// `s` without its `${secret:NAME}` references: they are the fix, never a secret, though `secret:NAME_OF_IT` reads as
/// `secret: <a long value>` to the scanner's generic pattern (the docs' own Linear trigger was refused).
fn without_secret_refs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find("${secret:") {
        out.push_str(&rest[..at]);
        if let Some(end) = rest[at..].find('}') {
            rest = &rest[at + end + 1..];
        } else {
            out.push_str(&rest[at..]);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

fn walk(v: &Value, path: &str, key: &str, secret: bool, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let scanned = without_secret_refs(s);
            // `wisp export` masked it (a token stored before plans checked for them): applying it as it is would put
            // the mask's text in place of the real token, and no pattern matches it any more (AAC-4)
            if s.contains(MASK) {
                let name = suggest_masked(s, key);
                out.push(format!("{path} is a masked value from wisp export: wisp secret set {name} and write ${{secret:{name}}}"));
            } else if let Some(hit) = secret_scan::scan_secrets(&scanned).first() {
                let name = suggest(hit.what, key);
                out.push(format!("{path} looks like {}: wisp secret set {name} and write ${{secret:{name}}}", hit.what));
            } else if secret {
                for name in env_refs(s) {
                    out.push(format!(
                        "{path}: ${{env:{name}}} would put its value in wisp's store: wisp secret set {name} and write ${{secret:{name}}}"
                    ));
                }
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                walk(x, &format!("{path}[{i}]"), key, secret, out);
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                walk(x, &format!("{path}.{k}"), k, secret, out);
            }
        }
        _ => {}
    }
}

/// Every secret written into `r`'s spec, as plan problems at its dotted path (`connectors.gh.headers.Authorization`).
pub fn problems(info: &KindInfo, r: &Resource) -> Vec<String> {
    let mut out = Vec::new();
    let at = format!("{}.{}", info.section, r.name);
    match &r.spec {
        Value::Object(m) => {
            for (k, x) in m {
                walk(x, &format!("{at}.{k}"), k, info.secret_fields.iter().any(|f| f == k), &mut out);
            }
        }
        other => walk(other, &at, &r.name, false, &mut out),
    }
    out
}

/// `v` with every string the scanner hits masked (`ghp_…(masked)`): what `wisp export` and `wisp plan --json` print.
pub fn mask(v: &mut Value) {
    match v {
        Value::String(s) => {
            if !secret_scan::find_secrets(s).is_empty() {
                *s = secret_scan::mask_secrets(s);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(mask),
        Value::Object(m) => m.values_mut().for_each(mask),
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn connector() -> KindInfo {
        KindInfo::new("connector", "connectors", "x", json!({})).secret_field("headers").secret_field("env")
    }

    /// A `${secret:NAME}` reference is the fix the plan asks for, never a problem, wherever it sits (a trigger's
    /// `secret: "${secret:LINEAR_WEBHOOK_SECRET}"` was refused as "a secret value"); a real value beside one still is.
    #[test]
    fn a_secret_reference_is_never_a_secret() {
        let trigger = KindInfo::new("trigger", "triggers", "x", json!({}));
        let r =
            Resource::new("trigger", "from-linear", json!({ "secret": "${secret:LINEAR_WEBHOOK_SECRET}", "header": "Linear-Signature" }));
        assert_eq!(problems(&trigger, &r), Vec::<String>::new());
        let r = Resource::new("connector", "x", json!({ "headers": { "Authorization": "Bearer ${secret:GH_TOKEN}" } }));
        assert_eq!(problems(&connector(), &r), Vec::<String>::new());
        let r = Resource::new("trigger", "x", json!({ "note": "${secret:A} api_key=abcdefghijklmnop1234" }));
        assert_eq!(problems(&trigger, &r).len(), 1, "a real value beside a reference is still caught");
        assert_eq!(without_secret_refs("a ${secret:B} c ${secret:unclosed"), "a  c ${secret:unclosed");
    }

    /// AAC-4: a literal token in a connector's headers or env was stored as it was, and exported to anyone.
    #[test]
    fn a_plaintext_token_is_a_problem_naming_its_path_and_fix_aac_4() {
        let gh = format!("Bearer ghp_{}", "a".repeat(36));
        let openai = format!("sk-proj-{}", "b".repeat(40));
        let r = Resource::new("connector", "gh", json!({ "headers": { "Authorization": gh }, "env": { "KEY": openai } }));
        assert_eq!(
            problems(&connector(), &r),
            [
                "connectors.gh.env.KEY looks like an OpenAI API key: wisp secret set OPENAI_API_KEY and write ${secret:OPENAI_API_KEY}",
                "connectors.gh.headers.Authorization looks like a GitHub token: wisp secret set GH_TOKEN and write ${secret:GH_TOKEN}",
            ]
        );
    }

    #[test]
    fn a_secret_reference_is_no_problem_and_env_in_a_secret_field_is_aac_4() {
        let ok =
            Resource::new("connector", "gh", json!({ "headers": { "Authorization": "Bearer ${secret:GH_TOKEN}" }, "url": "${env:HOST}" }));
        assert_eq!(problems(&connector(), &ok), Vec::<String>::new());
        let env = Resource::new("connector", "gh", json!({ "headers": { "Authorization": "Bearer ${env:GH_TOKEN}" } }));
        assert_eq!(
            problems(&connector(), &env),
            [
                "connectors.gh.headers.Authorization: ${env:GH_TOKEN} would put its value in wisp's store: wisp secret set GH_TOKEN and write ${secret:GH_TOKEN}"
            ]
        );
    }

    #[test]
    fn a_mask_hides_what_the_scanner_hits_aac_4() {
        let mut v = json!({ "headers": { "Authorization": format!("Bearer ghp_{}", "a".repeat(36)), "X": "${secret:K}" }, "n": 3 });
        mask(&mut v);
        assert_eq!(v, json!({ "headers": { "Authorization": "Bearer ghp_…(masked)", "X": "${secret:K}" }, "n": 3 }));
    }

    /// AAC-4: an export masks a token stored before plans checked for them; applying that export put the mask's text in
    /// place of the real token, since no pattern matches `ghp_…(masked)`.
    #[test]
    fn a_masked_value_from_an_export_is_a_problem_aac_4() {
        let mut spec = json!({
            "headers": { "Authorization": format!("Bearer ghp_{}", "a".repeat(36)) },
            "env": { "KEY": format!("sk-proj-{}", "b".repeat(40)) },
        });
        mask(&mut spec);
        let r = Resource::new("connector", "gh", spec);
        assert_eq!(
            problems(&connector(), &r),
            [
                "connectors.gh.env.KEY is a masked value from wisp export: wisp secret set OPENAI_API_KEY and write ${secret:OPENAI_API_KEY}",
                "connectors.gh.headers.Authorization is a masked value from wisp export: wisp secret set GH_TOKEN and write ${secret:GH_TOKEN}",
            ]
        );
    }
}
