//! Nushell completion: a module of an `extern` signature per command, which nushell also offers
//! as the subcommands of its parent; choices through completers.

use std::fmt::Write;

use super::Tree;
use crate::{Arity, Catalog, Param, ParamKind, Values};

/// The command a module exports under its own name.
const MAIN: &str = "main";

/// A completer: its name and the values it offers.
type Completer = (String, Vec<&'static str>);

impl Tree<'_> {
    pub(super) fn nu(&self) -> String {
        let mut body = String::new();
        let mut completers = Vec::new();
        for node in &self.nodes {
            let name =
                std::iter::once(self.program).chain(node.path.iter().copied()).collect::<Vec<_>>().join(" ");
            // The script is a module named as the program (`use vun.nu *`), whose command of its
            // own name is `main`.
            let signature = if node.path.is_empty() { MAIN } else { &name };
            let _ = writeln!(body, "# {}\nexport extern \"{signature}\" [", node.about);
            for param in node.positionals.iter().chain(&node.options) {
                let shape = param.nu_shape(&name, &mut completers);
                let _ = match param.kind {
                    ParamKind::Positional(Arity::One) => writeln!(body, "    {}: {shape}", param.name),
                    ParamKind::Positional(Arity::Optional) => writeln!(body, "    {}?: {shape}", param.name),
                    ParamKind::Positional(Arity::Many) => writeln!(body, "    ...{}: {shape}", param.name),
                    ParamKind::Flag | ParamKind::Option(_) => {
                        let short = param.short_form().map(|s| format!("({s})")).unwrap_or_default();
                        let value = if param.takes_value() { format!(": {shape}") } else { String::new() };
                        writeln!(body, "    {}{short}{value}  # {}", param.long_form(), param.about)
                    }
                };
            }
            body.push_str("]\n\n");
        }
        // Completers first, so the signatures after them can name them.
        let mut s = String::new();
        for (completer, names) in completers {
            let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
            let _ = writeln!(s, "def \"{completer}\" [] {{ [{}] }}", quoted.join(" "));
        }
        s.push('\n');
        s + &body
    }
}

impl Param {
    /// The shape of its values in the signature of `command`, adding a completer for choices.
    fn nu_shape(&self, command: &str, completers: &mut Vec<Completer>) -> String {
        match self.values {
            Values::Text | Values::Place => "string".to_owned(),
            Values::Count => "int".to_owned(),
            Values::Path(_) => "path".to_owned(),
            Values::Choice(names) | Values::Catalog(Catalog { names, .. }) => {
                let completer = format!("nu-complete {command} {}", self.name);
                completers.push((completer.clone(), names()));
                format!("string@\"{completer}\"")
            }
        }
    }
}
