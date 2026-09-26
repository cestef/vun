//! Bash completion, and zsh through its bash compatibility.

use std::fmt::Write;

use super::Tree;
use crate::parse::LONG;
use crate::{Param, Values};

/// Loads zsh's support for bash completion functions.
const ZSH_PRELUDE: &str = "autoload -U +X bashcompinit && bashcompinit\n";

impl Tree<'_> {
    pub(super) fn zsh(&self) -> String {
        [ZSH_PRELUDE, &self.bash()].concat()
    }

    pub(super) fn bash(&self) -> String {
        let program = self.program;
        let function: String = ["_", program]
            .concat()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let paths: Vec<String> = self.nodes.iter().skip(1).map(|n| format!("\"{}\"", n.key())).collect();
        let mut s = String::new();
        let _ = writeln!(s, "{function}() {{");
        s.push_str("    local cur=\"${COMP_WORDS[COMP_CWORD]}\" prev=\"${COMP_WORDS[COMP_CWORD-1]}\" path=\"\" i next\n");
        s.push_str("    for ((i = 1; i < COMP_CWORD; i++)); do\n");
        s.push_str("        next=\"${path:+$path }${COMP_WORDS[i]}\"\n");
        let _ = writeln!(s, "        case \"$next\" in {}) path=\"$next\" ;; esac", paths.join("|"));
        s.push_str("    done\n    case \"$path\" in\n");
        for node in &self.nodes {
            let _ = writeln!(s, "        \"{}\")", node.key());
            s.push_str("            case \"$prev\" in\n");
            for param in node.options.iter().filter(|p| p.takes_value()) {
                let _ = writeln!(
                    s,
                    "                {}) {} return ;;",
                    param.bash_spellings().join("|"),
                    param.values.bash_reply()
                );
            }
            s.push_str("            esac\n");
            let options: Vec<String> = node.options.iter().flat_map(|p| p.bash_spellings()).collect();
            let _ = writeln!(
                s,
                "            if [[ \"$cur\" == -* ]]; then COMPREPLY=($(compgen -W \"{}\" -- \"$cur\")); return; fi",
                options.join(" ")
            );
            let children = node.children.iter().map(|(name, _)| *name);
            let words: Vec<&str> = children.chain(node.positional_choices().unwrap_or_default()).collect();
            let files = if node.positional_paths() { " -f" } else { "" };
            let _ = writeln!(
                s,
                "            COMPREPLY=($(compgen{files} -W \"{}\" -- \"$cur\")) ;;",
                words.join(" ")
            );
        }
        s.push_str("    esac\n}\n");
        let _ = writeln!(s, "complete -o filenames -F {function} {program}");
        s
    }
}

impl Param {
    /// How it can be written: `--name`, and `-n` when it has a short form.
    fn bash_spellings(&self) -> Vec<String> {
        let long = format!("{LONG}{}", self.name);
        std::iter::once(long).chain(self.short.map(|c| format!("-{c}"))).collect()
    }
}

impl Values {
    /// The statement completing a value of these, ending in `;`.
    fn bash_reply(self) -> String {
        match self {
            Self::Text => "COMPREPLY=();".to_owned(),
            Self::Path(_) => "COMPREPLY=($(compgen -f -- \"$cur\"));".to_owned(),
            Self::Choice(names) => {
                format!("COMPREPLY=($(compgen -W \"{}\" -- \"$cur\"));", names().join(" "))
            }
        }
    }
}
