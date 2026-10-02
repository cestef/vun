//! Lint levels on the command line (spec 10, section 4): `--lint lint=level` overrides a build's
//! levels, and the session carries them to the lint engine as the input `LintLevelsOf`. A name
//! matching no lint or group of the packages a command covers is reported before anything is
//! checked.

use std::collections::HashSet;

use vun_command::{Args, Arity, Param, Values};
use vun_diag::{DiagId, Diagnostic, Msg, errors};
use vun_hir::db::PackageId;
use vun_sema::lint::{CheckedLints, Level, LintLevels, LintLevelsOf};
use vun_suggest::closest;

use crate::Session;
use crate::subject::Subject;

pub const LINTS: Param = Param::option("lint")
    .arity(Arity::Many)
    .about("raise or lower a lint or a group: `--lint std.lints.naming.case=warn`")
    .values(Values::Text);

/// Applies `--lint` to the session.
///
/// # Errors
/// Reports `cli.invalid_value` for an argument that is no `lint=level` or names no level.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    let pairs: Vec<(DiagId, Level)> = args
        .values(&LINTS)
        .filter_map(|given| {
            let Some((name, level)) = given.split_once('=') else {
                session.diag.error(no_level(given));
                return None;
            };
            let Some(level) = Level::of_name(level) else {
                session.diag.error(no_name(level));
                return None;
            };
            Some((DiagId::owned(name.to_owned()), level))
        })
        .collect();
    if !pairs.is_empty() {
        session.add_lint_levels(LintLevels::of(pairs));
    }
    Ok(())
}

/// The report for an argument that is not `lint=level`.
fn no_level(given: &str) -> Diagnostic {
    let msg = Msg::new().code(given).text(" is no lint level: write ").code("lint=level");
    let help = Msg::new().text("for example ").code("--lint std.lints.naming.case=warn");
    Diagnostic::error(errors::CLI_INVALID_VALUE, msg).help(help)
}

/// The report for a level that is none of `lang.lint.Level`'s.
fn no_name(level: &str) -> Diagnostic {
    let msg = Msg::new().code(level).text(" is no lint level");
    let d = Diagnostic::error(errors::CLI_INVALID_VALUE, msg);
    match closest(level, Level::names()) {
        Some(near) => d.help(Msg::did_you_mean(near)),
        None => d.help(Msg::new().text("a level is ").code_choices(Level::names())),
    }
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
            known.extend(checked.lints.iter().map(|(id, _)| id.clone()));
            known.extend(checked.groups.iter().cloned());
        }
        let names: Vec<String> = known.iter().map(ToString::to_string).collect();
        let unknown: Vec<DiagId> =
            levels.iter().filter(|(id, _)| !known.contains(id)).map(|(id, _)| id.clone()).collect();
        for id in &unknown {
            let name = id.to_string();
            let msg = Msg::new().text("no lint or group is named ").code(name.clone());
            let mut d = Diagnostic::error(errors::LINT_UNKNOWN, msg);
            if let Some(near) = closest(&name, names.iter().map(String::as_str)) {
                d = d.help(Msg::did_you_mean(near));
            }
            self.diag.error(d);
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
