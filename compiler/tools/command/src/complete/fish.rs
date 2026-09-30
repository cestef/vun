//! Fish completion: one `complete` line per command, option and value list.

use std::fmt::Write;

use super::Tree;
use crate::{Catalog, Param, Values};

impl Tree<'_> {
    pub(super) fn fish(&self) -> String {
        let program = self.program;
        let function = format!("__{}_path", program.replace(' ', "_"));
        let paths: Vec<String> = self.nodes.iter().skip(1).map(|n| Self::fish_quote(&n.key())).collect();
        let mut s = String::new();
        // The words selecting a command, among those typed so far.
        let _ = writeln!(s, "function {function}");
        s.push_str("    set -l path\n    for word in (commandline -opc)[2..-1]\n");
        s.push_str("        set -l next (string join -- ' ' $path $word)\n");
        let _ = writeln!(s, "        if contains -- $next {}", paths.join(" "));
        s.push_str(
            "            set path $path $word\n        end\n    end\n    string join -- ' ' $path\nend\n",
        );
        let _ = writeln!(s, "complete -c {program} -f");
        for node in &self.nodes {
            let at = format!("-n 'test \"$({function})\" = \"{}\"'", node.key());
            for (child, about) in &node.children {
                let _ = writeln!(s, "complete -c {program} {at} -a {child} -d {}", Self::fish_quote(about));
            }
            for param in &node.options {
                let _ = writeln!(
                    s,
                    "complete -c {program} {at} {} -d {}",
                    param.fish_flags(),
                    Self::fish_quote(param.about)
                );
            }
            if let Some(names) = node.positional_choices() {
                let _ = writeln!(s, "complete -c {program} {at} -a {}", Self::fish_quote(&names.join(" ")));
            } else if node.positional_paths() {
                let _ = writeln!(s, "complete -c {program} {at} -F");
            }
        }
        s
    }

    /// `text` in single quotes.
    fn fish_quote(text: &str) -> String {
        format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

impl Param {
    /// The flags of `complete` declaring this option and its values.
    fn fish_flags(&self) -> String {
        let short = self.short.map(|c| format!(" -s {c}")).unwrap_or_default();
        let value = match (self.takes_value(), self.values) {
            (false, _) => String::new(),
            (true, Values::Text | Values::Count) => " -x".to_owned(),
            (true, Values::Path(_)) => " -r -F".to_owned(),
            (true, Values::Choice(names) | Values::Catalog(Catalog { names, .. })) => {
                format!(" -x -a {}", Tree::fish_quote(&names().join(" ")))
            }
        };
        format!("-l {}{short}{value}", self.name)
    }
}
