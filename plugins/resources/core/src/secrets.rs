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

fn walk(v: &Value, path: &str, key: &str, secret: bool, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            if let Some(hit) = secret_scan::scan_secrets(s).first() {
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
}
