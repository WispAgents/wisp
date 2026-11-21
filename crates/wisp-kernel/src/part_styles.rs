//! `contributes.partStyles` (ADR-0007): a trusted theme's styles, by `data-part`, which the shell writes into
//! `@layer plugins`. The allow-list is `packages/shell/src/theme-styles/part-styles.json`, the same file the
//! shell's compiler reads, so what passes here is exactly what the app applies (and nothing else does):
//!   - a key is a part name (`composer.send`: dot-namespaced, lowercase), `:root` (token overrides: custom
//!     properties only), or a theme group `@light` / `@dark` holding the same map once;
//!   - a property is on the list or a custom property (`--accent`);
//!   - a value is plain CSS text: no `;`, braces, `<`, `>`, `\`, `@`, `!`, comments, control characters or
//!     unbalanced quotes (nothing can close the rule or open another), `url()` only with a `data:image/` URL and
//!     no `image-set()` / `src()` (nothing is fetched from a remote host).

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::{Map, Value};

const ALLOW_JSON: &str = include_str!("../../../packages/shell/src/theme-styles/part-styles.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Allow {
    properties: Vec<String>,
    groups: Vec<String>,
    tokens: String,
    max_value: usize,
    max_name: usize,
    max_rules: usize,
}

fn allow() -> Option<&'static Allow> {
    static A: OnceLock<Option<Allow>> = OnceLock::new();
    A.get_or_init(|| serde_json::from_str(ALLOW_JSON).ok()).as_ref()
}

fn name_ok(s: &str, max: usize) -> bool {
    s.len() <= max && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.'))
}

/// `composer.send`, `ui.button`: a namespaced part name, safe inside `[data-part~="…"]`.
pub fn part_ok(s: &str) -> bool {
    let max = allow().map_or(64, |a| a.max_name);
    name_ok(s, max)
        && s.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.contains('.')
        && s.split('.').all(|g| !g.is_empty())
}

fn custom_ok(s: &str) -> bool {
    let max = allow().map_or(64, |a| a.max_name);
    s.strip_prefix("--").is_some_and(|r| {
        !r.is_empty() && r.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()) && name_ok(r, max) && !r.contains('.')
    })
}

/// May a theme set `prop`? Under `:root` only custom properties.
pub fn property_ok(prop: &str, tokens: bool) -> bool {
    custom_ok(prop) || (!tokens && allow().is_some_and(|a| a.properties.iter().any(|p| p == prop)))
}

/// Why `v` can't be a value, or `None` when it can.
pub fn value_problem(v: &str) -> Option<&'static str> {
    let max = allow().map_or(200, |a| a.max_value);
    if v.trim().is_empty() || v.len() > max {
        return Some("a value is 1 to 200 characters");
    }
    if v.chars().any(|c| c.is_control() || matches!(c, ';' | '{' | '}' | '<' | '>' | '\\' | '@' | '!'))
        || v.contains("/*")
        || v.contains("*/")
    {
        return Some("a value can't hold ; { } < > \\ @ ! or a comment");
    }
    if v.matches('"').count() % 2 == 1 || v.matches('\'').count() % 2 == 1 {
        return Some("a value's quotes must be balanced");
    }
    let low = v.to_ascii_lowercase();
    if low.contains("image-set(") || low.contains("src(") || low.contains("expression(") {
        return Some("a value can't fetch anything (image-set, src)");
    }
    let mut rest = low.as_str();
    while let Some(i) = rest.find("url(") {
        let arg = rest[i + 4..].trim_start().trim_start_matches(['"', '\'']);
        if !arg.starts_with("data:image/") {
            return Some("url() takes only a data:image/ URL: a theme fetches nothing from a remote host");
        }
        rest = &rest[i + 4..];
    }
    None
}

fn decls(v: &Value, at: &str, tokens: bool, out: &mut Vec<(String, String)>) -> usize {
    let Some(m) = v.as_object() else {
        out.push((at.to_owned(), "must map properties to values".into()));
        return 0;
    };
    for (prop, val) in m {
        let here = format!("{at}.{prop}");
        if !property_ok(prop, tokens) {
            let why = if tokens { "under :root only custom properties (--accent) may be set" } else { "isn't a property a theme may set" };
            out.push((here, why.into()));
        } else if let Some(s) = val.as_str() {
            if let Some(why) = value_problem(s) {
                out.push((here, why.into()));
            }
        } else {
            out.push((here, "must be a string".into()));
        }
    }
    m.len()
}

fn rules(m: &Map<String, Value>, at: &str, nested: bool, out: &mut Vec<(String, String)>) -> usize {
    let Some(a) = allow() else {
        out.push((at.to_owned(), "the part-styles allow-list didn't read".into()));
        return 0;
    };
    let mut n = 0;
    for (k, v) in m {
        let here = format!("{at}[{k:?}]");
        if a.groups.iter().any(|g| g == k) {
            match v.as_object() {
                Some(g) if !nested => n += rules(g, &here, true, out),
                _ => out.push((here, "a theme group (@light, @dark) maps parts to styles, once".into())),
            }
        } else if *k == a.tokens {
            n += decls(v, &here, true, out);
        } else if part_ok(k) {
            n += decls(v, &here, false, out);
        } else {
            out.push((here, "must be a part name (composer.send), :root, @light or @dark".into()));
        }
    }
    n
}

/// Every problem with a `partStyles` value: `(path below contributes.partStyles, message)`.
pub fn check(v: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(m) = v.as_object() else {
        return vec![(String::new(), "must map part names to {property: value}".into())];
    };
    let n = rules(m, "", false, &mut out);
    if let Some(a) = allow()
        && n > a.max_rules
    {
        out.push((String::new(), format!("at most {} declarations", a.max_rules)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_allow_list_reads() {
        let a = allow().expect("part-styles.json reads");
        assert!(a.properties.iter().any(|p| p == "background"));
        assert_eq!(a.groups, ["@light", "@dark"]);
    }

    #[test]
    fn a_theme_with_parts_tokens_and_dark_values_passes() {
        let ok = json!({
            "composer.send": { "background": "var(--accent)", "border-radius": "999px" },
            ":root": { "--accent": "#e0457b" },
            "@dark": { ":root": { "--accent": "#ff7aa8" }, "ui.button": { "color": "rgb(255 255 255 / 0.9)" } },
            "list.row": { "font-family": "\"Iosevka\", monospace", "background-image": "url(\"data:image/svg+xml;utf8,x\")" }
        });
        // `;` inside the data URL is refused: a data URL is base64 or plain, never a second declaration
        let problems = check(&ok);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].0.contains("background-image"));
    }

    #[test]
    fn injections_are_refused() {
        for bad in [
            "red; } body { display: none",
            "red } * { color: red",
            "red !important",
            "url(https://evil.example/x.png)",
            "url( 'http://evil.example/x' )",
            "image-set(\"https://evil.example/a.png\" 1x)",
            "red /* */",
            "@import url(x)",
            "\"unbalanced",
            "</style><script>",
            "a\\7d b",
            "",
        ] {
            assert!(value_problem(bad).is_some(), "{bad:?} must be refused");
        }
        assert!(value_problem("url(data:image/png;base64,AAAA)").is_some(), "a ; can't pass even in a data URL");
        assert!(value_problem("url(data:image/png,AAAA)").is_none());
    }

    #[test]
    fn names_properties_and_groups_are_checked() {
        let p = check(&json!({
            "x\"] body [y": { "color": "red" },
            "nodot": { "color": "red" },
            "composer.send": { "position": "fixed", "--ok": "1px" },
            ":root": { "color": "red" },
            "@dark": { "@light": {} },
            "@sepia": {}
        }));
        let paths: Vec<&str> = p.iter().map(|(a, _)| a.as_str()).collect();
        assert_eq!(p.len(), 6, "{paths:?}");
        assert!(part_ok("composer.send") && part_ok("ui.button.label") && !part_ok("composer") && !part_ok("Composer.send"));
    }
}
