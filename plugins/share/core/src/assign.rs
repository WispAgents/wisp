//! Secrets typed inline in a shell command (audit G GS4): `DB_PASSWORD=hunter2 npm run migrate`,
//! `export API_TOKEN=…`, `--password value`, `mysql -phunter2`. The value of an assignment or a flag whose name looks
//! secret becomes `<secret>`; the name stays, so the reader still sees what was set. A reference (`$TOKEN`,
//! `${TOKEN}`) isn't a secret and is kept.

/// Whether an env var or flag name looks like it holds a secret: one of its parts (split on `_`, `-`, `.`) holds
/// `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `PASSWD`, `PASSPHRASE` or `CREDENTIAL`, ends in `PASS` (`DB_PASS`, `PGPASS`;
/// not `BYPASS` or `COMPASS`), or is `AUTH` / `AUTHORIZATION`. The share scrub's env rule (`secret_env`) is this one.
pub fn secret_name(name: &str) -> bool {
    name.split(|c: char| !c.is_ascii_alphanumeric()).filter(|p| !p.is_empty()).any(|p| {
        let p = p.to_ascii_uppercase();
        ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PASSPHRASE", "CREDENTIAL"].iter().any(|w| p.contains(w))
            || (p.ends_with("PASS") && !matches!(p.as_str(), "BYPASS" | "COMPASS" | "SURPASS" | "OVERPASS"))
            || p == "AUTH"
            || p.starts_with("AUTHORIZATION")
    })
}

/// A token of a shell line: where it starts and ends (bytes). Quotes keep spaces inside one token.
fn tokens(s: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some(&(i, c)) = it.peek() {
        if c.is_whitespace() {
            it.next();
            continue;
        }
        let start = i;
        let mut end = s.len();
        let mut quote: Option<char> = None;
        while let Some(&(j, c)) = it.peek() {
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == '"' || c == '\'' {
                quote = Some(c);
            } else if c.is_whitespace() {
                end = j;
                break;
            }
            it.next();
        }
        out.push((start, end));
    }
    out
}

/// A value worth masking: something, and not a reference to another variable or already masked.
fn real(v: &str) -> bool {
    let v = v.trim_matches(|c| c == '"' || c == '\'');
    !v.is_empty() && !v.starts_with('$') && v != "<secret>" && !v.starts_with("<env:")
}

/// `NAME=` at the start of `t` with a shell identifier for a name: the name.
fn assigned(t: &str) -> Option<&str> {
    let (name, _) = t.split_once('=')?;
    let mut cs = name.chars();
    (cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(name)
}

/// Tools whose short flag takes a password: (tool, flag, whether the value may come as the next word). `mysql -p db`
/// reads `db` as the database (the password is prompted), so only `-pVALUE` is one there.
const SHORT: [(&str, &str, bool); 7] = [
    ("mysql", "-p", false),
    ("mysqldump", "-p", false),
    ("mysqladmin", "-p", false),
    ("mariadb", "-p", false),
    ("mariadb-dump", "-p", false),
    ("sshpass", "-p", true),
    ("redis-cli", "-a", true),
];

/// `text` with every secret typed inline masked, and how many. `command`: a command line (flags count too), else
/// output, where only `NAME=value` counts (an `env` listing, a `.env` echoed).
pub fn mask_inline(text: &str, command: bool) -> (String, usize) {
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for line in line_spans(text) {
        let l = &text[line.0..line.1];
        let toks = tokens(l);
        let mut tool: Option<&str> = None;
        let mut next_is_secret = false;
        for (k, &(a, b)) in toks.iter().enumerate() {
            let t = &l[a..b];
            let at = line.0 + a;
            if std::mem::take(&mut next_is_secret) && real(t) {
                cuts.push((at, at + t.len()));
                continue;
            }
            if matches!(t, "|" | "||" | "&&" | ";" | "&") {
                tool = None;
                continue;
            }
            if let Some(name) = assigned(t) {
                let v = &t[name.len() + 1..];
                if secret_name(name) && real(v) {
                    cuts.push((at + name.len() + 1, at + t.len()));
                }
                continue;
            }
            if !command {
                continue;
            }
            if let Some(flag) = t.strip_prefix("--").filter(|f| !f.is_empty() && !f.starts_with("no-")) {
                match flag.split_once('=') {
                    Some((n, v)) if secret_name(n) && real(v) => cuts.push((at + 2 + n.len() + 1, at + t.len())),
                    None if secret_name(flag) => next_is_secret = toks.get(k + 1).is_some_and(|&(c, d)| !l[c..d].starts_with('-')),
                    _ => {}
                }
                continue;
            }
            if tool.is_none() && !matches!(t, "sudo" | "env" | "export" | "exec" | "command" | "time") {
                tool = Some(t.rsplit(['/', '\\']).next().unwrap_or(t));
                continue;
            }
            if let Some(&(_, f, spaced)) = SHORT.iter().find(|(n, _, _)| Some(*n) == tool)
                && let Some(rest) = t.strip_prefix(f)
            {
                if !rest.is_empty() && real(rest) {
                    cuts.push((at + f.len(), at + t.len()));
                } else if rest.is_empty() && spaced {
                    next_is_secret = true;
                }
            }
        }
    }
    if cuts.is_empty() {
        return (text.to_owned(), 0);
    }
    let mut out = String::with_capacity(text.len());
    let mut done = 0;
    for (a, b) in &cuts {
        out.push_str(&text[done..*a]);
        out.push_str("<secret>");
        done = *b;
    }
    out.push_str(&text[done..]);
    (out, cuts.len())
}

fn line_spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if c == '\n' {
            out.push((start, i));
            start = i + 1;
        }
    }
    out.push((start, text.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(s: &str) -> String {
        mask_inline(s, true).0
    }

    #[test]
    fn a_secret_assigned_inline_is_masked_and_its_name_kept() {
        assert_eq!(cmd("DB_PASSWORD=hunter2qa npm run migrate"), "DB_PASSWORD=<secret> npm run migrate");
        assert_eq!(cmd("export API_TOKEN='abc def'"), "export API_TOKEN=<secret>");
        assert_eq!(cmd("env AWS_SECRET_ACCESS_KEY=x1 GH_AUTH=y2 make"), "env AWS_SECRET_ACCESS_KEY=<secret> GH_AUTH=<secret> make");
        assert_eq!(cmd("PGPASS=p1 DB_PASS=p2 psql"), "PGPASS=<secret> DB_PASS=<secret> psql");
        assert_eq!(cmd("set -x; MY_CREDENTIALS=c ./run"), "set -x; MY_CREDENTIALS=<secret> ./run");
    }

    #[test]
    fn names_that_only_look_alike_and_references_are_kept() {
        for keep in
            ["NODE_ENV=production npm start", "BYPASS=1 make", "AUTHOR=ada git commit", "TOKEN=$GITHUB_TOKEN gh pr list", "PWD=/x ls"]
        {
            assert_eq!(cmd(keep), keep);
        }
    }

    #[test]
    fn secret_flags_are_masked_where_the_tool_takes_the_value() {
        assert_eq!(cmd("psql --password hunter2 db"), "psql --password <secret> db");
        assert_eq!(cmd("deploy --api-key=k123 --verbose"), "deploy --api-key=<secret> --verbose");
        assert_eq!(cmd("tool --token --dry-run"), "tool --token --dry-run", "a flag after it is no value");
        assert_eq!(cmd("mysql -u root -phunter2 shop"), "mysql -u root -p<secret> shop");
        assert_eq!(cmd("mysql -u root -p shop"), "mysql -u root -p shop", "mysql -p db prompts: db is the database");
        assert_eq!(cmd("sshpass -p hunter2 ssh box"), "sshpass -p <secret> ssh box");
        assert_eq!(cmd("/usr/bin/redis-cli -a s3cret ping"), "/usr/bin/redis-cli -a <secret> ping");
        assert_eq!(cmd("grep -p x file"), "grep -p x file", "-p means a password only to the tools that say so");
    }

    #[test]
    fn output_masks_assignments_but_not_flags() {
        let (out, n) = mask_inline("DB_PASSWORD=hunter2\nPATH=/bin\nusage: --password PASS", false);
        assert_eq!((out.as_str(), n), ("DB_PASSWORD=<secret>\nPATH=/bin\nusage: --password PASS", 1));
    }
}
