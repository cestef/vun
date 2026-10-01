//! What a package's plan says of the options of commands (spec 12, section 2): the values to use
//! where neither the command line nor the environment gives one, set by `b.commands`.

use std::path::Path;

use vun_build::layout::MANIFEST;
use vun_command::{Setting, Settings};
use vun_origin::OriginId;
use vun_sema_plan::{Action, Planned};
use vun_sema_ty::Value;

use crate::Session;

impl Session {
    /// What the plan of the package around `path` (this directory with none) gives the options of
    /// commands, and where the plan is. None where there is no package, or its plan does not run,
    /// which the command reports on its own.
    pub fn settings(&mut self, path: Option<&Path>) -> Option<(Settings, OriginId)> {
        let here = std::env::current_dir().ok()?;
        let dir = match path {
            Some(path) if path.is_dir() => path.to_path_buf(),
            Some(path) => path.parent().map_or_else(|| here.clone(), Path::to_path_buf),
            None => here,
        };
        let (package, _) = self.package_around(&dir)?;
        let file = self.load(&package.join(MANIFEST))?;
        let key = self.file_ref(file);
        let plan = self.db.get::<Planned>(&key)?.ok()?;
        let steps = Action::steps(&plan).ok()?;
        let mut settings = Vec::new();
        for step in steps {
            if let Action::Commands(value) = step {
                sections(&value, &mut settings);
            }
        }
        Some((Settings::new(settings), plan.at))
    }
}

/// The settings `value` holds, a record of records: its fields are commands, theirs options.
fn sections(value: &Value, into: &mut Vec<Setting>) {
    let Some(commands) = fields(value) else { return };
    for (command, section) in commands {
        let Some(options) = fields(section) else { continue };
        for (option, given) in options {
            let mut values = Vec::new();
            if texts(given, &mut values) {
                into.push(Setting {
                    command: command.as_str().to_owned(),
                    option: option.as_str().to_owned(),
                    values,
                });
            }
        }
    }
}

/// The fields of a record, or of the one a value of `lang.meta.Any` holds.
fn fields(value: &Value) -> Option<&[(vun_intern::Symbol, Value)]> {
    match value {
        Value::Any { value, .. } => fields(value),
        Value::Struct { fields, .. } => Some(fields),
        _ => None,
    }
}

/// The values `value` stands for as text, a number as its digits, a name or a variant as its
/// name, a string as it is, a tuple or an array as each of its items; whether it is such.
fn texts(value: &Value, into: &mut Vec<String>) -> bool {
    match value {
        Value::Int(n) => into.push(n.to_string()),
        Value::Variant { name, .. } | Value::Name(name) => into.push(name.as_str().to_owned()),
        Value::Any { value, .. } => return texts(value, into),
        Value::Array(items) | Value::Tuple(items) => match value.text() {
            Some(text) => into.push(text),
            None => return items.iter().all(|item| texts(item, into)),
        },
        _ => return false,
    }
    true
}
