//! `wisp secret set|rm NAME`: a secret a formula references as `${secret:NAME}` (a connector's token, a webhook's
//! key), kept in the secrets file, never in a formula or the store (`resources.secretSet`). The value never goes on
//! the command line, where the shell's history would keep it: it's piped in (`printf %s "$T" | wisp secret set N`)
//! or typed at a prompt that doesn't echo it.

use std::io::{IsTerminal, Read, Write};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal;
use serde_json::json;

use crate::client::{CliError, Client, Result};
use crate::scripted::say;

/// The value: stdin when it's piped (its last newline dropped), else typed with nothing shown.
pub(crate) fn value(name: &str) -> Result<String> {
    let mut stdin = std::io::stdin();
    if !stdin.is_terminal() {
        let mut v = String::new();
        stdin.read_to_string(&mut v)?;
        // a trailing newline (echo, a file) isn't part of the secret
        return Ok(v.strip_suffix('\n').map_or(v.as_str(), |l| l.strip_suffix('\r').unwrap_or(l)).to_owned());
    }
    let mut out = std::io::stderr();
    let _ = write!(out, "{name}: ");
    let _ = out.flush();
    terminal::enable_raw_mode()?;
    let typed = read_hidden();
    let _ = terminal::disable_raw_mode();
    let _ = writeln!(out);
    typed
}

fn read_hidden() -> Result<String> {
    let mut v = String::new();
    loop {
        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Enter => return Ok(v),
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => return Err(CliError::said("cancelled")),
            KeyCode::Backspace => {
                v.pop();
            }
            KeyCode::Char(c) => v.push(c),
            _ => {}
        }
    }
}

/// `wisp secret set NAME` / `wisp secret rm NAME`.
pub async fn run(c: &Client, words: &[String]) -> Result<i32> {
    let (verb, name) = match words {
        [v, n] if v == "set" || v == "rm" || v == "remove" => (v.as_str(), n.as_str()),
        _ => return Err(CliError::said("usage: wisp secret set NAME   |   wisp secret rm NAME")),
    };
    if verb == "set" {
        let v = value(name)?;
        c.cmd("resources.secretSet", json!({ "name": name, "value": v })).await?;
        say(&format!("  ✓ {name} is set here: formulas can reference it as ${{secret:{name}}}"));
    } else {
        c.cmd("resources.secretRemove", json!({ "name": name })).await?;
        say(&format!("  ✓ {name} is removed"));
    }
    Ok(0)
}
