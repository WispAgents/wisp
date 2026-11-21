//! The docs' examples are tested (ADR-0039): every page under `docs/site` with yaml examples gets them as one
//! `.wisp/` folder (each block a file, as a reader would split them), and `wisp validate` on a real `wisp serve` must
//! pass: the schema, the references, the expressions. A doc that shows a field wisp doesn't take fails here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{start_with, temp};

fn pages(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            // the generated reference has no examples to check
            if p.file_name().is_some_and(|n| n != "reference") {
                pages(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "md" || x == "mdx") {
            out.push(p);
        }
    }
}

/// What marks a block wisp can't check offline (a module fetched from a git URL): an HTML comment the reader
/// doesn't see, on the line before the block.
const UNCHECKED: &str = "<!-- example: unchecked -->";

/// The page's yaml code blocks, but those marked [`UNCHECKED`].
fn yaml_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("```yaml\n") {
        let marked = rest[..at].trim_end().ends_with(UNCHECKED);
        let body = &rest[at + 8..];
        let Some(end) = body.find("```") else { break };
        if !marked {
            out.push(body[..end].to_owned());
        }
        rest = &body[end + 3..];
    }
    out
}

/// The `${secret:NAME}` names in a block.
fn secret_names(block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(at) = rest.find("${secret:") {
        let tail = &rest[at + 9..];
        let end = tail.find('}').unwrap_or(tail.len());
        out.push(tail[..end].to_owned());
        rest = &tail[end..];
    }
    out
}

fn set_secret(dir: &Path, port: u16, name: &str) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(["secret", "set", name])
        .env("WISP_HOME", dir.join("home"))
        .env("WISP_PORT", port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    std::io::Write::write_all(c.stdin.as_mut().unwrap(), b"example-value").unwrap();
    drop(c.stdin.take());
    assert!(c.wait().unwrap().success(), "wisp secret set {name}");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_formula_example_in_the_docs_validates() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/site");
    let mut all = Vec::new();
    pages(&docs, &mut all);
    all.sort();
    let dir = temp("serve-docs");
    let (serve, port) = start_with(&dir, &[]);
    let mut checked = 0;
    let mut failed = Vec::new();
    for page in &all {
        let blocks = yaml_blocks(&std::fs::read_to_string(page).unwrap());
        if blocks.is_empty() {
            continue;
        }
        let repo = dir.join(format!("page-{checked}"));
        let wisp_dir = repo.join(".wisp");
        std::fs::create_dir_all(&wisp_dir).unwrap();
        for (i, b) in blocks.iter().enumerate() {
            std::fs::write(wisp_dir.join(format!("{i:02}.wisp.yml")), b).unwrap();
            // the secrets an example names are each reader's own: set as a reader would (`wisp secret set`)
            for name in secret_names(b) {
                set_secret(&dir, port, &name);
            }
        }
        let o = Command::new(env!("CARGO_BIN_EXE_wisp"))
            .arg("validate")
            .env("WISP_HOME", dir.join("home"))
            .env("WISP_PORT", port.to_string())
            .env("NO_COLOR", "1")
            .current_dir(&repo)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        checked += 1;
        if !o.status.success() {
            let said = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            failed.push(format!("{}:\n{}", page.strip_prefix(&docs).unwrap_or(page).display(), said.trim()));
        }
    }
    drop(serve);
    assert!(checked > 0, "no page with a ```yaml example under docs/site");
    assert!(failed.is_empty(), "{} of {checked} pages' formula examples don't validate:\n\n{}", failed.len(), failed.join("\n\n"));
}
