//! `wisp completions zsh|bash|fish`: the scripts, written from the same catalogue as the verbs (the
//! curated commands, every plugin and its verbs, each verb's flags).

use std::fmt::Write as _;

use crate::verbs::Catalogue;

fn q(s: &str) -> String {
    s.replace('\'', "")
}

/// A plugin's first word and its verbs, each with its flags.
type Tree = Vec<(String, Vec<(String, Vec<String>)>)>;

/// (word, its verbs as words, each verb's flags) for every plugin.
fn tree(c: &Catalogue) -> Tree {
    c.plugins()
        .into_iter()
        .map(|(p, verbs)| {
            let vs = verbs.iter().map(|v| (v.words.join("-"), v.spec.fields.iter().map(|f| format!("--{}", f.flag())).collect())).collect();
            (p.to_owned(), vs)
        })
        .collect()
}

/// What the catalogue doesn't say: the built-in groups' verbs (`wisp plugin enable`, `wisp cloud signin`), the
/// harnesses `--agent` takes, and the setting keys (setup audit F27). Empty: the catalogue alone.
#[derive(Debug, Clone, Default)]
pub struct Extra {
    pub groups: Vec<(String, Vec<String>)>,
    pub agents: Vec<String>,
    pub settings: Vec<String>,
    /// each enum setting's values, after its key (`wisp settings set worktrees.max <TAB>`: off 20 40 80). Additive
    /// (2026-11-19, setup audit B10).
    pub values: Vec<(String, Vec<String>)>,
    /// the words a top-level verb takes (`wisp export <TAB>`: the kinds, then its flags). Additive (2026-11-19,
    /// setup audit E).
    pub after: Vec<(String, Vec<String>)>,
    /// the goals declared by name (`goals:` in a formula, ADR-0110), after `--goal` in every shell (owner-list #63).
    /// Additive (2026-11-20).
    pub goals: Vec<String>,
}

/// The script for `shell`, or None for a shell it doesn't know.
pub fn script(shell: &str, c: &Catalogue, curated: &[&str]) -> Option<String> {
    script_with(shell, c, curated, &Extra::default())
}

/// [`script`], with what the catalogue doesn't say ([`Extra`]). Additive (2026-11-19).
pub fn script_with(shell: &str, c: &Catalogue, curated: &[&str], extra: &Extra) -> Option<String> {
    match shell {
        "zsh" => Some(zsh(c, curated, extra)),
        "bash" => Some(bash(c, curated, extra)),
        "fish" => Some(fish(c, curated, extra)),
        _ => None,
    }
}

/// Each word once, in the order first seen.
fn once(words: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in words {
        if !out.contains(&w) {
            out.push(w);
        }
    }
    out
}

fn bash(c: &Catalogue, curated: &[&str], extra: &Extra) -> String {
    let t = tree(c);
    let mut s = String::from(
        "# wisp completions for bash: eval \"$(wisp completions bash)\"\n_wisp() {\n  local cur=${COMP_WORDS[COMP_CWORD]} prev=${COMP_WORDS[COMP_CWORD-1]} p=${COMP_WORDS[1]} v=${COMP_WORDS[2]}\n",
    );
    // what a flag takes, after it: the harnesses after `--agent`, the declared goals after `--goal`
    let mut takes = String::new();
    if !extra.agents.is_empty() {
        let _ = write!(takes, "\n    --agent|-a) COMPREPLY=($(compgen -W '{}' -- \"$cur\")); return ;;", q(&extra.agents.join(" ")));
    }
    if !extra.goals.is_empty() {
        let _ = write!(takes, "\n    --goal) COMPREPLY=($(compgen -W '{}' -- \"$cur\")); return ;;", q(&extra.goals.join(" ")));
    }
    if !takes.is_empty() {
        let _ = writeln!(s, "  case \"$prev\" in{takes}\n  esac");
    }
    s.push_str("  if [ \"$COMP_CWORD\" -eq 1 ]; then\n");
    let firsts = once(
        curated
            .iter()
            .map(|x| (*x).to_owned())
            .chain(t.iter().map(|(p, _)| p.clone()))
            .chain(extra.groups.iter().map(|(g, _)| g.clone()))
            .chain(["help".into(), "completions".into(), "attach".into()]),
    );
    let _ = writeln!(s, "    COMPREPLY=($(compgen -W '{}' -- \"$cur\")); return\n  fi\n  case \"$p\" in", q(&firsts.join(" ")));
    // every group: its built-in verbs, then its plugin's, each once
    let names = once(extra.groups.iter().map(|(g, _)| g.clone()).chain(t.iter().map(|(p, _)| p.clone())));
    for p in &names {
        let built: Vec<String> = extra.groups.iter().filter(|(g, _)| g == p).flat_map(|(_, vs)| vs.clone()).collect();
        let plugin: &[(String, Vec<String>)] = t.iter().find(|(x, _)| x == p).map_or(&[], |(_, vs)| vs.as_slice());
        let verbs = once(built.into_iter().chain(plugin.iter().map(|(v, _)| v.clone())));
        let _ = writeln!(
            s,
            "    {p}) if [ \"$COMP_CWORD\" -eq 2 ]; then COMPREPLY=($(compgen -W '{}' -- \"$cur\")); return; fi\n      case \"$v\" in",
            q(&verbs.join(" "))
        );
        for (v, flags) in plugin {
            // `wisp settings set <key>` and `--key <key>`: the setting keys
            if p == "settings" && matches!(v.as_str(), "set" | "get") && !extra.settings.is_empty() {
                let keys = q(&extra.settings.join(" "));
                // `wisp settings set <key> <TAB>`: that setting's values, when it has a list
                let mut vals = String::new();
                if v == "set" && !extra.values.is_empty() {
                    vals.push_str("; elif [ \"$COMP_CWORD\" -eq 4 ]; then case \"${COMP_WORDS[3]}\" in");
                    for (k, vs) in &extra.values {
                        let _ = write!(vals, " {}) COMPREPLY=($(compgen -W '{}' -- \"$cur\")); return ;;", q(k), q(&vs.join(" ")));
                    }
                    vals.push_str(" esac; COMPREPLY=($(compgen -W '--json --help' -- \"$cur\"))");
                }
                let _ = writeln!(
                    s,
                    "        {v}) if [ \"$COMP_CWORD\" -eq 3 ] || [ \"$prev\" = --key ]; then COMPREPLY=($(compgen -W '{keys}' -- \"$cur\")){vals}; else COMPREPLY=($(compgen -W '{} --json --help' -- \"$cur\")); fi ;;",
                    q(&flags.join(" "))
                );
            } else {
                let _ = writeln!(s, "        {v}) COMPREPLY=($(compgen -W '{} --json --help' -- \"$cur\")) ;;", q(&flags.join(" ")));
            }
        }
        s.push_str("      esac ;;\n");
    }
    for (verb, words) in extra.after.iter().filter(|(v, _)| !names.contains(v)) {
        let _ = writeln!(s, "    {verb}) COMPREPLY=($(compgen -W '{} --help' -- \"$cur\")) ;;", q(&words.join(" ")));
    }
    s.push_str("    completions) COMPREPLY=($(compgen -W 'zsh bash fish' -- \"$cur\")) ;;\n  esac\n}\ncomplete -F _wisp wisp\n");
    s
}

fn zsh(c: &Catalogue, curated: &[&str], extra: &Extra) -> String {
    let t = tree(c);
    let mut s = String::from(
        "#compdef wisp\n# wisp completions for zsh: wisp completions zsh > \"${fpath[1]}/_wisp\"\n_wisp() {\n  local -a firsts verbs flags\n",
    );
    let firsts: Vec<String> = curated
        .iter()
        .map(|x| format!("'{x}:a wisp command'"))
        .chain(t.iter().map(|(p, v)| format!("'{p}:{} commands'", v.len())))
        .chain(["'help:every command'".into(), "'completions:shell completions'".into(), "'attach:follow a task live'".into()])
        .collect();
    let _ = writeln!(s, "  firsts=({})\n  if (( CURRENT == 2 )); then _describe 'command' firsts; return; fi", firsts.join(" "));
    // after `--goal`: the declared goals, whatever the verb
    if !extra.goals.is_empty() {
        let names: Vec<String> = extra.goals.iter().map(|g| format!("'{}'", q(g))).collect();
        let _ = writeln!(s, "  if [[ $words[CURRENT-1] == --goal ]]; then compadd -- {}; return; fi", names.join(" "));
    }
    s.push_str("  case $words[2] in\n");
    for (p, verbs) in &t {
        let names: Vec<String> = verbs.iter().map(|(v, _)| format!("'{}'", q(v))).collect();
        let _ = writeln!(
            s,
            "    {p})\n      verbs=({})\n      if (( CURRENT == 3 )); then _describe 'verb' verbs; return; fi\n      case $words[3] in",
            names.join(" ")
        );
        for (v, flags) in verbs {
            let _ = writeln!(s, "        {v}) flags=({} --json --help) ;;", flags.join(" "));
        }
        s.push_str("      esac\n      _describe 'flag' flags ;;\n");
    }
    for (verb, words) in extra.after.iter().filter(|(v, _)| !t.iter().any(|(p, _)| p == v)) {
        let _ = writeln!(s, "    {verb}) compadd -- {} ;;", words.iter().map(|w| q(w)).collect::<Vec<_>>().join(" "));
    }
    s.push_str("    completions) _values 'shell' zsh bash fish ;;\n  esac\n}\ncompdef _wisp wisp\n");
    s
}

fn fish(c: &Catalogue, curated: &[&str], extra: &Extra) -> String {
    let t = tree(c);
    let mut s =
        String::from("# wisp completions for fish: wisp completions fish > ~/.config/fish/completions/wisp.fish\ncomplete -c wisp -f\n");
    for x in curated.iter().chain(["help", "completions", "attach"].iter()) {
        let _ = writeln!(s, "complete -c wisp -n '__fish_use_subcommand' -a '{x}'");
    }
    for (p, verbs) in &t {
        let _ = writeln!(s, "complete -c wisp -n '__fish_use_subcommand' -a '{p}' -d '{} commands'", verbs.len());
        for (v, flags) in verbs {
            let _ = writeln!(s, "complete -c wisp -n '__fish_seen_subcommand_from {p}; and not __fish_seen_subcommand_from {v}' -a '{v}'");
            for f in flags {
                let _ = writeln!(s, "complete -c wisp -n '__fish_seen_subcommand_from {v}' -l '{}'", f.trim_start_matches("--"));
            }
        }
    }
    for (verb, words) in &extra.after {
        let (flags, plain): (Vec<&String>, Vec<&String>) = words.iter().partition(|w| w.starts_with("--"));
        let plain: Vec<String> = plain.iter().map(|w| q(w)).collect();
        let _ = writeln!(s, "complete -c wisp -n '__fish_seen_subcommand_from {verb}' -a '{}'", plain.join(" "));
        for f in flags {
            let _ = writeln!(s, "complete -c wisp -n '__fish_seen_subcommand_from {verb}' -l '{}'", q(f.trim_start_matches("--")));
        }
    }
    let _ = writeln!(s, "complete -c wisp -n '__fish_seen_subcommand_from completions' -a 'zsh bash fish'");
    // after `--goal`: the declared goals (-r: it takes one, -f: no files)
    if !extra.goals.is_empty() {
        let names: Vec<String> = extra.goals.iter().map(|g| q(g)).collect();
        let _ = writeln!(s, "complete -c wisp -l goal -r -f -a '{}' -d 'a declared goal'", names.join(" "));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_kernel::CommandInfo;

    #[test]
    fn each_shell_names_every_plugin_verb_and_flag() {
        let c = Catalogue::new(vec![CommandInfo {
            id: "crew.create".into(),
            plugin: "wisp.crew".into(),
            title: None,
            read_only: false,
            permission: "crew:write".into(),
            args: Some("{ name?: string, }".into()),
            ui_only: None,
        }]);
        for sh in ["zsh", "bash", "fish"] {
            let s = script(sh, &c, &["ls", "show"]).unwrap();
            assert!(s.contains("crew") && s.contains("create") && s.contains("name") && s.contains("ls"), "{sh}: {s}");
        }
        assert!(script("tcsh", &c, &[]).is_none());
    }

    /// Setup audit F27: bash offered `plugins` twice, nothing after `wisp plugin` or `wisp cloud`, no harness after
    /// `--agent`, and no setting keys after `wisp settings set`.
    #[test]
    fn bash_offers_each_word_once_the_built_in_verbs_the_harnesses_and_the_setting_keys() {
        let info = |id: &str| CommandInfo {
            id: id.into(),
            plugin: format!("wisp.{}", id.split('.').next().unwrap_or_default()),
            title: None,
            read_only: false,
            permission: "x".into(),
            args: Some("{ key?: string, value?: string, }".into()),
            ui_only: None,
        };
        let c = Catalogue::new(vec![info("plugins.list"), info("settings.set"), info("settings.get")]);
        let extra = Extra {
            groups: vec![
                ("plugin".into(), vec!["enable".into(), "disable".into(), "install".into()]),
                ("cloud".into(), vec!["signin".into(), "status".into()]),
            ],
            agents: vec!["claude".into(), "codex".into()],
            settings: vec!["worktrees.max".into(), "budget.limitHold".into()],
            values: Vec::new(),
            after: Vec::new(),
            goals: Vec::new(),
        };
        let s = script_with("bash", &c, &["plugin", "plugins", "attach"], &extra).unwrap();
        let firsts =
            s.lines().find(|l| l.contains("COMP_CWORD\" -eq 1") || l.contains("compgen -W 'plugin")).unwrap_or_default().to_owned();
        let words: Vec<&str> =
            s.split("compgen -W '").nth(1).unwrap_or_default().split('\'').next().unwrap_or_default().split(' ').collect();
        let mut sorted = words.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), words.len(), "each first word once: {words:?} {firsts}");
        assert!(s.contains("    plugin) ") && s.contains("enable disable install"), "{s}");
        assert!(s.contains("    cloud) ") && s.contains("signin status"), "{s}");
        assert!(s.contains("--agent|-a) COMPREPLY=($(compgen -W 'claude codex'"), "{s}");
        assert!(s.contains("worktrees.max budget.limitHold"), "{s}");
    }

    /// Setup audit B10: `wisp settings set worktrees.max <TAB>` offered `--key --value --json --help`, not off 20 40 80.
    #[test]
    fn bash_offers_an_enum_settings_values_after_its_key() {
        let info = |id: &str| CommandInfo {
            id: id.into(),
            plugin: "wisp.settings".into(),
            title: None,
            read_only: false,
            permission: "x".into(),
            args: Some("{ key?: string, value?: string, }".into()),
            ui_only: None,
        };
        let c = Catalogue::new(vec![info("settings.set"), info("settings.get")]);
        let extra = Extra {
            settings: vec!["worktrees.max".into(), "settings.editor".into()],
            values: vec![("worktrees.max".into(), vec!["off".into(), "20".into(), "40".into(), "80".into()])],
            ..Extra::default()
        };
        let s = script_with("bash", &c, &[], &extra).unwrap();
        assert!(s.contains("-eq 4 ]; then case \"${COMP_WORDS[3]}\" in worktrees.max) COMPREPLY=($(compgen -W 'off 20 40 80'"), "{s}");
        let get = s.lines().find(|l| l.trim_start().starts_with("get)")).unwrap();
        assert!(!get.contains("off 20 40 80"), "only set takes a value: {get}");
    }

    /// Audit E (as in D): `wisp export <TAB>` offered nothing, though it takes the kinds (`wisp export crew agents`). A
    /// word list after a top-level verb (`Extra::after`) is offered there, in every shell.
    #[test]
    fn export_offers_the_kinds_in_every_shell() {
        let c = Catalogue::new(Vec::new());
        let words = ["projects", "crew", "settings", "--format", "--out"].map(str::to_owned).to_vec();
        let extra = Extra { after: vec![("export".into(), words)], ..Extra::default() };
        let bash = script_with("bash", &c, &["export"], &extra).unwrap();
        assert!(
            bash.contains("    export) COMPREPLY=($(compgen -W 'projects crew settings --format --out --help' -- \"$cur\")) ;;"),
            "{bash}"
        );
        let zsh = script_with("zsh", &c, &["export"], &extra).unwrap();
        assert!(zsh.contains("    export) compadd -- projects crew settings --format --out ;;"), "{zsh}");
        let fish = script_with("fish", &c, &["export"], &extra).unwrap();
        assert!(fish.contains("complete -c wisp -n '__fish_seen_subcommand_from export' -a 'projects crew settings'"), "{fish}");
        assert!(fish.contains("complete -c wisp -n '__fish_seen_subcommand_from export' -l 'format'"), "{fish}");
    }

    /// Owner-list #63: `wisp new … --goal <TAB>` completes the goals declared by name, in bash, zsh and fish; with none
    /// declared nothing is added, and `--agent` still completes the harnesses beside it.
    #[test]
    fn goal_completes_the_declared_goal_names_in_every_shell() {
        let c = Catalogue::new(Vec::new());
        let extra = Extra { agents: vec!["claude".into()], goals: vec!["green-build".into(), "docs-clean".into()], ..Extra::default() };
        let bash = script_with("bash", &c, &["new"], &extra).unwrap();
        assert!(bash.contains("    --goal) COMPREPLY=($(compgen -W 'green-build docs-clean' -- \"$cur\")); return ;;"), "{bash}");
        assert!(bash.contains("    --agent|-a) COMPREPLY=($(compgen -W 'claude'"), "{bash}");
        let zsh = script_with("zsh", &c, &["new"], &extra).unwrap();
        assert!(zsh.contains("  if [[ $words[CURRENT-1] == --goal ]]; then compadd -- 'green-build' 'docs-clean'; return; fi"), "{zsh}");
        let fish = script_with("fish", &c, &["new"], &extra).unwrap();
        assert!(fish.contains("complete -c wisp -l goal -r -f -a 'green-build docs-clean' -d 'a declared goal'"), "{fish}");
        for sh in ["bash", "zsh", "fish"] {
            let none = script_with(sh, &c, &["new"], &Extra::default()).unwrap();
            assert!(!none.contains("--goal") && !none.contains("-l goal"), "{sh}: {none}");
        }
    }
}
