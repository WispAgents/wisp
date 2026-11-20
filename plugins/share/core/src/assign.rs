//! Secrets typed inline (audit G GS4, review r194): `DB_PASSWORD=hunter2 npm run migrate`, `export API_TOKEN=…`,
//! `--password value`, `mysql -phunter2`, `curl -u user:pass`, `docker login -p …`, a token echoed into
//! `--with-token`, and the shapes a secret takes in any text (shares too): a URL's `user:pass@`, `Basic …`, a JSON
//! field named like one, a `.netrc` password, `aws configure set aws_secret_access_key …`, npm's `:_authToken=`. The
//! secret becomes `<secret>` and the structure stays (`user:<secret>@host`, `"password":"<secret>"`), so the reader
//! still sees what was set. A reference (`$TOKEN`, `${TOKEN}`) isn't a secret and is kept, and so is a value that
//! plainly isn't one (a number, on/off, a URL, a path to a key file).

use std::sync::OnceLock;

use regex::{Captures, Regex};

/// A name part that is a secret by itself (`API_KEY`, `apiKey`, `GH_TOKEN`, `MYSQL_PWD`).
const SECRET_PARTS: [&str; 18] = [
    "KEY",
    "APIKEY",
    "ACCESSKEY",
    "SECRETKEY",
    "PRIVATEKEY",
    "TOKEN",
    "AUTHTOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PASSPHRASE",
    "PASS",
    "PAT",
    "CREDENTIAL",
    "CREDENTIALS",
    "AUTH",
    "AUTHORIZATION",
    "PWD",
];

/// A name part a secret word ends (`GHTOKEN`, `PGPASSWORD`, `CLIENTSECRET`, `DBPASS`).
const SECRET_ENDS: [&str; 5] = ["TOKEN", "SECRET", "PASSWORD", "PASSWD", "APIKEY"];

/// A name part that makes the name about a secret, not the secret: where it is (`--auth-url`, `SSH_AUTH_SOCK`,
/// `API_KEY_FILE`), what kind, how many (`--max-tokens`), the program that asks for it (`GIT_ASKPASS`), or how it's
/// read (`--password-stdin`).
const CONTEXT_PARTS: [&str; 36] = [
    "URL",
    "URI",
    "ENDPOINT",
    "HOST",
    "HOSTS",
    "PORT",
    "TYPE",
    "METHOD",
    "MODE",
    "FILE",
    "FILES",
    "PATH",
    "DIR",
    "SOCK",
    "SOCKET",
    "STDIN",
    "LENGTH",
    "LEN",
    "COUNT",
    "SIZE",
    "LIMIT",
    "MAX",
    "MIN",
    "ID",
    "NAME",
    "HELPER",
    "CMD",
    "COMMAND",
    "PROGRAM",
    "ASKPASS",
    "PARALLELISM",
    "ENABLED",
    "DISABLED",
    "TIMEOUT",
    "TTL",
    "EXPIRY",
];

/// A name's parts, upper case: split on anything not a letter or digit, and at a lower-to-upper change (`apiKey`).
fn parts(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() {
            out.push(std::mem::take(&mut cur));
            prev_lower = false;
            continue;
        }
        if c.is_ascii_uppercase() && prev_lower {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        cur.push(c.to_ascii_uppercase());
    }
    out.push(cur);
    out.retain(|p| !p.is_empty());
    out
}

/// Whether an env var or flag name holds a secret: a whole part is one ([`SECRET_PARTS`]) or ends in one
/// ([`SECRET_ENDS`], `…PASS` but not `BYPASS`/`COMPASS`/`ASKPASS`), and no part says it's about one instead
/// ([`CONTEXT_PARTS`]). `PWD` alone is the working folder. The share scrub's env rule (`secret_env`) is this one.
pub fn secret_name(name: &str) -> bool {
    let ps = parts(name);
    if ps.iter().any(|p| CONTEXT_PARTS.contains(&p.as_str())) {
        return false;
    }
    ps.iter().any(|p| {
        (SECRET_PARTS.contains(&p.as_str()) && !(p == "PWD" && ps.len() == 1))
            || SECRET_ENDS.iter().any(|w| p.ends_with(w))
            || (p.ends_with("PASS") && !matches!(p.as_str(), "BYPASS" | "COMPASS" | "SURPASS" | "OVERPASS" | "ASKPASS"))
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

fn unquoted(v: &str) -> &str {
    v.trim_matches(|c| c == '"' || c == '\'')
}

/// A value worth masking: something, and not a reference to another variable or already masked.
fn real(v: &str) -> bool {
    let v = unquoted(v);
    !v.is_empty() && !v.starts_with('$') && v != "<secret>" && !v.starts_with("<env:")
}

/// A value that plainly isn't a secret, whatever its name says: on/off and the like, a number, a URL with no
/// credentials in it, a path (`--key ./server.key`, `--private-key ~/.ssh/id_ed25519`).
fn plain(v: &str) -> bool {
    let v = unquoted(v);
    let l = v.to_ascii_lowercase();
    matches!(l.as_str(), "on" | "off" | "true" | "false" | "yes" | "no" | "none" | "null")
        || (!v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        || ((l.starts_with("http://") || l.starts_with("https://")) && !v.contains('@'))
        || v.starts_with('/')
        || v.starts_with("~/")
        || v.starts_with("./")
        || v.starts_with("../")
}

/// A value to mask: real, and not plainly something else.
fn masks(v: &str) -> bool {
    real(v) && !plain(v)
}

/// `NAME=` at the start of `t` with a shell identifier for a name: the name.
fn assigned(t: &str) -> Option<&str> {
    let (name, _) = t.split_once('=')?;
    let mut cs = name.chars();
    (cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(name)
}

/// Tools whose short flag takes a password: (tool, its subcommand if it needs one, flag, whether the value may come as
/// the next word). `mysql -p db` reads `db` as the database (the password is prompted), so only `-pVALUE` is one there;
/// `docker -p` is a port except under `login`.
const SHORT: [(&str, Option<&str>, &str, bool); 8] = [
    ("mysql", None, "-p", false),
    ("mysqldump", None, "-p", false),
    ("mysqladmin", None, "-p", false),
    ("mariadb", None, "-p", false),
    ("mariadb-dump", None, "-p", false),
    ("sshpass", None, "-p", true),
    ("redis-cli", None, "-a", true),
    ("docker", Some("login"), "-p", true),
];

/// Tools whose `-u` / `--user` is `user:password`: the part after the colon is masked.
const USER_PASS: [&str; 2] = ["curl", "wget"];

/// Flags that read a secret from stdin: an `echo`/`printf` piped into one hands it a secret typed inline.
const FROM_STDIN: [&str; 3] = ["--with-token", "--password-stdin", "--token-stdin"];

const WRAPPERS: [&str; 6] = ["sudo", "env", "export", "exec", "command", "time"];

/// One command of a line (between `|`, `&&`, `;`): the tool, its first word after it, and the tokens.
struct Seg {
    toks: Vec<(usize, usize)>,
    piped_out: bool,
}

/// The cuts (byte spans to mask) of one line's tokens.
fn line_cuts(l: &str, base: usize, command: bool, cuts: &mut Vec<(usize, usize)>) {
    let all = tokens(l);
    let mut segs: Vec<Seg> = vec![Seg { toks: Vec::new(), piped_out: false }];
    for &(a, b) in &all {
        let t = &l[a..b];
        if matches!(t, "|" | "||" | "&&" | ";" | "&") {
            if let Some(last) = segs.last_mut() {
                last.piped_out = t == "|";
            }
            segs.push(Seg { toks: Vec::new(), piped_out: false });
        } else if let Some(s) = segs.last_mut() {
            s.toks.push((a, b));
        }
    }
    let cut = |cuts: &mut Vec<(usize, usize)>, a: usize, b: usize| cuts.push((base + a, base + b));
    for (si, seg) in segs.iter().enumerate() {
        let mut tool: Option<&str> = None;
        let mut sub: Option<&str> = None;
        let mut next_is_secret = false;
        let mut next_is_userpass = false;
        for (k, &(a, b)) in seg.toks.iter().enumerate() {
            let t = &l[a..b];
            if std::mem::take(&mut next_is_secret) && masks(t) {
                cut(cuts, a, b);
                continue;
            }
            if std::mem::take(&mut next_is_userpass) {
                if let Some(c) = unquoted(t).find(':').map(|c| c + usize::from(t.starts_with(['"', '\''])))
                    && masks(&t[c + 1..])
                {
                    cut(cuts, a + c + 1, b - usize::from(t.ends_with(['"', '\''])));
                }
                continue;
            }
            if let Some(name) = assigned(t) {
                if secret_name(name) && masks(&t[name.len() + 1..]) {
                    cut(cuts, a + name.len() + 1, b);
                }
                continue;
            }
            if !command {
                continue;
            }
            let user_pass = tool.is_some_and(|x| USER_PASS.contains(&x));
            if let Some(flag) = t.strip_prefix("--").filter(|f| !f.is_empty() && !f.starts_with("no-")) {
                let (n, v) = flag.split_once('=').map_or((flag, None), |(n, v)| (n, Some(v)));
                match v {
                    Some(v) if user_pass && n == "user" => {
                        if let Some(c) = v.find(':')
                            && masks(&v[c + 1..])
                        {
                            cut(cuts, a + 2 + n.len() + 1 + c + 1, b);
                        }
                    }
                    Some(v) if secret_name(n) && masks(v) => cut(cuts, a + 2 + n.len() + 1, b),
                    None if user_pass && n == "user" => next_is_userpass = true,
                    None if secret_name(n) => {
                        next_is_secret = seg.toks.get(k + 1).is_some_and(|&(c, d)| !l[c..d].starts_with('-'));
                    }
                    _ => {}
                }
                continue;
            }
            if tool.is_none() && !WRAPPERS.contains(&t) {
                tool = Some(t.rsplit(['/', '\\']).next().unwrap_or(t));
                continue;
            }
            if user_pass && t == "-u" {
                next_is_userpass = true;
                continue;
            }
            if let Some(&(_, _, f, spaced)) = SHORT.iter().find(|(n, s, _, _)| Some(*n) == tool && (s.is_none() || *s == sub))
                && let Some(rest) = t.strip_prefix(f)
            {
                if !rest.is_empty() && masks(rest) {
                    cut(cuts, a + f.len(), b);
                } else if rest.is_empty() && spaced {
                    next_is_secret = true;
                }
                continue;
            }
            if sub.is_none() && !t.starts_with('-') {
                sub = Some(t);
            }
        }
        // `echo <token> | gh auth login --with-token`: what's echoed is the secret
        let reads_stdin = |s: &Seg| s.toks.iter().any(|&(a, b)| FROM_STDIN.contains(&&l[a..b]));
        let echoes = seg.toks.first().is_some_and(|&(a, b)| matches!(&l[a..b], "echo" | "printf"));
        if command && echoes && seg.piped_out && segs.get(si + 1).is_some_and(reads_stdin) {
            for &(a, b) in seg.toks.iter().skip(1).filter(|&&(a, b)| !l[a..b].starts_with('-')) {
                if masks(&l[a..b]) {
                    cut(cuts, a, b);
                }
            }
        }
    }
}

/// The shapes a secret takes in any text, each with what of it stays: (pattern, how a match is masked).
/// How a shape's match is masked: the text it becomes, or None to leave it.
type Masked = fn(&Captures<'_>) -> Option<String>;
type Shape = (Regex, Masked);

fn shapes() -> &'static [Shape] {
    static S: OnceLock<Vec<Shape>> = OnceLock::new();
    S.get_or_init(|| {
        let list: [(&str, Masked); 8] = [
            // a URL's `user:password@`: `https://user:<secret>@host`
            (r#"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@'"]+:)([^\s@/'"]+)@"#, |c| real(&c[2]).then(|| format!("{}<secret>@", &c[1]))),
            // a token as a URL's user: `https://<secret>@github.com/…` (a name is short; a token isn't)
            (r"(?i)\b([a-z][a-z0-9+.-]*://)([A-Za-z0-9_.~+-]{20,})@", |c| Some(format!("{}<secret>@", &c[1]))),
            // HTTP Basic after its header: `Authorization: Basic <secret>` ("basic setup" in prose stays)
            (r#"(?i)(\bauthorization\s*:?\s*["']?\s*basic\s+)([A-Za-z0-9+/]{8,}={0,2})"#, |c| Some(format!("{}<secret>", &c[1]))),
            // a JSON field named like a secret: `"password":"<secret>"`
            (r#""([A-Za-z0-9_.-]{1,64})"(\s*:\s*)"((?:[^"\\]|\\.)+)""#, |c| {
                (secret_name(&c[1]) && masks(&c[3])).then(|| format!("\"{}\"{}\"<secret>\"", &c[1], &c[2]))
            }),
            // `.netrc`: `machine host login user password <secret>`
            (r"(?i)(\bmachine\s+\S+(?:\s+\S+){0,4}?\s+password\s+)(\S+)", |c| real(&c[2]).then(|| format!("{}<secret>", &c[1]))),
            // `aws configure set aws_secret_access_key <secret>`
            (r"(?i)(\baws\s+configure\s+set\s+(\S+)\s+)(\S+)", |c| {
                (secret_name(&c[2]) && masks(&c[3])).then(|| format!("{}<secret>", &c[1]))
            }),
            // npm's `//registry.npmjs.org/:_authToken=<secret>` (and `_auth`, `_password`)
            (r"(?i)(:_(?:authToken|auth|password)\s*=\s*)(\S+)", |c| real(&c[2]).then(|| format!("{}<secret>", &c[1]))),
            // GitLab's tokens (`glpat-…`, `glptt-…`), which the share's shapes didn't know
            (r"\b(gl(?:pat|ptt|dt|rt)-)[A-Za-z0-9_-]{20,}", |_| Some("<secret>".to_owned())),
        ];
        list.into_iter().filter_map(|(p, f)| Regex::new(p).ok().map(|r| (r, f))).collect()
    })
}

/// The shapes' patterns, for the share's one-pass check of whether a string has anything to take out.
pub(crate) fn shape_patterns() -> impl Iterator<Item = &'static str> {
    shapes().iter().map(|(r, _)| r.as_str())
}

/// `text` with every secret shape masked, keeping its structure; each masked match as it was.
pub(crate) fn mask_shapes(text: &str) -> (String, Vec<String>) {
    let mut t = text.to_owned();
    let mut found = Vec::new();
    for (re, f) in shapes() {
        if !re.is_match(&t) {
            continue;
        }
        t = re
            .replace_all(&t, |c: &Captures<'_>| match f(c) {
                Some(now) => {
                    found.push(c[0].to_owned());
                    now
                }
                None => c[0].to_owned(),
            })
            .into_owned();
    }
    (t, found)
}

/// `text` with every secret typed inline masked, and how many. `command`: a command line (flags, tools, pipes count
/// too), else output, where `NAME=value` and the shapes count (an `env` listing, a `.env` or `.netrc` printed).
pub fn mask_inline(text: &str, command: bool) -> (String, usize) {
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for (a, b) in line_spans(text) {
        line_cuts(&text[a..b], a, command, &mut cuts);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = String::with_capacity(text.len());
    let mut done = 0;
    for (a, b) in &cuts {
        if *a < done {
            continue;
        }
        out.push_str(&text[done..*a]);
        out.push_str("<secret>");
        done = *b;
    }
    out.push_str(&text[done..]);
    let (out, found) = mask_shapes(&out);
    (out, cuts.len() + found.len())
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

    /// Review r194 of GS4: what still went out as typed. Each keeps its structure.
    #[test]
    fn credentials_in_urls_headers_bodies_and_tools_are_masked_review_r194() {
        for (said, want) in [
            ("curl -u ada:hunter2qa https://api.x.dev", "curl -u ada:<secret> https://api.x.dev"),
            ("curl --user=ada:hunter2qa https://api.x.dev", "curl --user=ada:<secret> https://api.x.dev"),
            ("git clone https://ada:hunter2qa@git.x.dev/r.git", "git clone https://ada:<secret>@git.x.dev/r.git"),
            ("git push https://glpat-abcdefghijklmnopqrstu@gitlab.com/a/b.git", "git push https://<secret>@gitlab.com/a/b.git"),
            ("git remote add o https://ghp_abcdefghij0123456789@github.com/a/b", "git remote add o https://<secret>@github.com/a/b"),
            (r#"curl -H "Authorization: Basic YWRhOmh1bnRlcjI=" x"#, r#"curl -H "Authorization: Basic <secret>" x"#),
            (r#"curl --data '{"user":"ada","password":"hunter2qa"}' x"#, r#"curl --data '{"user":"ada","password":"<secret>"}' x"#),
            ("aws configure set aws_secret_access_key wJalrXUtnFEMI", "aws configure set aws_secret_access_key <secret>"),
            ("docker login -u ada -p hunter2qa ghcr.io", "docker login -u ada -p <secret> ghcr.io"),
            ("docker login --password hunter2qa ghcr.io", "docker login --password <secret> ghcr.io"),
            ("MYSQL_PWD=hunter2qa mysql shop", "MYSQL_PWD=<secret> mysql shop"),
            ("echo abcdef0123456789 | gh auth login --with-token", "echo <secret> | gh auth login --with-token"),
            ("echo $T | gh auth login --with-token", "echo $T | gh auth login --with-token"),
            ("npm config set //registry.npmjs.org/:_authToken=npm_abcdef", "npm config set //registry.npmjs.org/:_authToken=<secret>"),
        ] {
            assert_eq!(cmd(said), want, "{said}");
        }
        let (netrc, n) = mask_inline("machine api.x.dev login ada password hunter2qa", false);
        assert_eq!((netrc.as_str(), n), ("machine api.x.dev login ada password <secret>", 1));
    }

    /// Review r194 of GS4: names that only contain KEY/TOKEN/AUTH, and values that plainly aren't secrets, were masked.
    #[test]
    fn what_isnt_a_secret_stays_as_typed_review_r194() {
        for keep in [
            "TOKENIZERS_PARALLELISM=false python train.py",
            "llm --max-tokens 4000 hi",
            "jq --sort-keys . a.json",
            "gpg --recv-keys 0xABCDEF12",
            "KEYBOARD_LAYOUT=dvorak setxkbmap",
            "ssh-add --apple-use-keychain ~/.ssh/id_ed25519",
            "curl --key ./client.key --cert ./client.crt https://x.dev",
            "tool --private-key ~/.ssh/id_ed25519",
            "kc --auth-url https://sso.x.dev/auth",
            "AUTH=off make",
            "docker login --password-stdin ghcr.io",
            "docker run -p 8080:80 nginx",
            "GIT_ASKPASS=/usr/lib/git-core/askpass git fetch",
            "SSH_AUTH_SOCK=/tmp/ssh-x/agent.1 ssh box",
            "AWS_ACCESS_KEY_ID=AKIAEXAMPLE aws s3 ls",
        ] {
            assert_eq!(cmd(keep), keep);
        }
        for (name, secret) in [
            ("API_KEY", true),
            ("apiKey", true),
            ("GH_TOKEN", true),
            ("NPM_TOKEN", true),
            ("PGPASSWORD", true),
            ("CLIENT_SECRET", true),
            ("MYSQL_PWD", true),
            ("PWD", false),
            ("GIT_ASKPASS", false),
            ("VSCODE_GIT_ASKPASS_MAIN", false),
            ("SSH_AUTH_SOCK", false),
            ("TOKENIZERS_PARALLELISM", false),
            ("max-tokens", false),
            ("sort-keys", false),
            ("KEYBOARD_LAYOUT", false),
            ("password-stdin", false),
        ] {
            assert_eq!(secret_name(name), secret, "{name}");
        }
    }
}
