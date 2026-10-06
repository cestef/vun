//! Lint levels on the command line (spec 10, section 4): `--lint lint=level` overrides a build's
//! levels, and the session carries them to the lint engine as the input `LintLevelsOf`. A name
//! matching no lint or group of the packages a command covers is reported before anything is
//! checked.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use vun_command::{Args, Arity, Param, Values};
use vun_diag::{DiagId, Diagnostic, Msg, errors};
use vun_hir::db::PackageId;
use vun_sema_lint::{CheckedLints, Level, LintLevels, LintLevelsOf};
use vun_suggest::closest;

use crate::Session;
use crate::subject::Subject;

pub const LINTS: Param = Param::option("lint")
    .arity(Arity::Many)
    .about("raise or lower a lint or a group: `--lint std.lints.naming.case=warn`")
    .values(Values::Text);

/// A `--lint` argument that sets no level: what is wrong with it, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    /// The argument is no `lint=level`.
    NoLevel(String),
    /// The level is none of `lang.lint.Level`'s.
    NoName(String),
    /// The lint or group is none the command covers, with the closest that is.
    Unknown { id: DiagId, near: Option<DiagId> },
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLevel(given) => write!(f, "`{given}` is no lint level"),
            Self::NoName(level) => write!(f, "`{level}` is no lint level"),
            Self::Unknown { id, .. } => write!(f, "no lint or group is named `{id}`"),
        }
    }
}

impl Error for Invalid {}

impl Invalid {
    /// The report of it.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::NoLevel(given) => {
                let msg = Msg::new().code(given).text(" is no lint level: write ").code("lint=level");
                let help = Msg::new().text("for example ").code("--lint std.lints.naming.case=warn");
                Diagnostic::error(errors::CLI_INVALID_VALUE, msg).help(help)
            }
            Self::NoName(level) => {
                let msg = Msg::new().code(level).text(" is no lint level");
                let d = Diagnostic::error(errors::CLI_INVALID_VALUE, msg);
                match closest(level, Level::names()) {
                    Some(near) => d.help(Msg::did_you_mean(near)),
                    None => d.help(Msg::new().text("a level is ").code_choices(Level::names())),
                }
            }
            Self::Unknown { id, near } => {
                let msg = Msg::new().text("no lint or group is named ").code(id.to_string());
                let d = Diagnostic::error(errors::LINT_UNKNOWN, msg);
                match near {
                    Some(near) => d.help(Msg::did_you_mean(near.to_string())),
                    None => d,
                }
            }
        }
    }
}

/// A lint or group and the level a `--lint` argument gives it.
struct Setting {
    id: DiagId,
    level: Level,
}

impl Setting {
    /// Reads `given`, written `lint=level`.
    fn read(given: &str) -> Result<Self, Invalid> {
        let (name, level) = given.split_once('=').ok_or_else(|| Invalid::NoLevel(given.to_owned()))?;
        let level = Level::of_name(level).ok_or_else(|| Invalid::NoName(level.to_owned()))?;
        Ok(Self { id: DiagId::parse(name), level })
    }
}

/// Applies `--lint` to the session.
///
/// # Errors
/// Reports `cli.invalid_value` for an argument that is no `lint=level` or names no level.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    let settings = args.values(&LINTS).filter_map(|given| match Setting::read(given) {
        Ok(setting) => Some((setting.id, setting.level)),
        Err(invalid) => {
            session.diag.error(invalid.diagnostic());
            None
        }
    });
    let pairs: Vec<(DiagId, Level)> = settings.collect();
    if !pairs.is_empty() {
        session.add_lint_levels(LintLevels::of(pairs));
    }
    Ok(())
}

impl Session {
    /// Sets the lint levels the command line gives.
    pub fn add_lint_levels(&mut self, levels: LintLevels) {
        self.db.set::<LintLevelsOf>(&(), levels);
    }

    /// The lint levels the command line gives.
    #[must_use]
    pub fn lint_levels(&self) -> LintLevels {
        self.db.get::<LintLevelsOf>(&())
    }

    /// Reports each `--lint` naming no lint or group of the packages `subject` covers; whether
    /// every name is known.
    pub fn resolve_lints(&mut self, subject: &Subject) -> bool {
        let levels = self.lint_levels();
        if levels.is_empty() {
            return true;
        }
        let packages = self.subject_packages(subject);
        let mut known: HashSet<DiagId> = HashSet::new();
        for package in &packages {
            let checked = self.db.get::<CheckedLints>(package);
            known.extend(checked.lints.iter().map(|(id, _)| *id));
            known.extend(checked.groups.iter().copied());
        }
        let names: Vec<String> = known.iter().map(ToString::to_string).collect();
        let unknown = levels.iter().filter(|(id, _)| !known.contains(id)).map(|(id, _)| *id);
        let unknown: Vec<Invalid> = unknown
            .map(|id| {
                let near = closest(&id.to_string(), names.iter().map(String::as_str)).map(DiagId::parse);
                Invalid::Unknown { id, near }
            })
            .collect();
        for invalid in &unknown {
            self.diag.error(invalid.diagnostic());
        }
        // The command line configures the code the command names, not the libraries it reaches.
        let scoped = self.lint_levels().scoped_to(packages);
        self.add_lint_levels(scoped);
        unknown.is_empty()
    }

    /// The packages `subject` covers, published: the file's own for a loose file.
    fn subject_packages(&mut self, subject: &Subject) -> Vec<PackageId> {
        let dirs = subject.packages();
        if dirs.is_empty() {
            let loose = self.loose();
            return vec![self.publish(loose)];
        }
        let keys: Vec<_> = dirs.iter().filter_map(|dir| self.package_at(dir)).collect();
        keys.into_iter().map(|key| self.publish(key)).collect()
    }
}
