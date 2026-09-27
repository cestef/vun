//! The checks of unsafe code at run time a session's programs are built with (spec 07, section
//! 7): `--checks` names entries of `lang.check.Kind`, and a program of a plan adds those of its
//! `Options.checks`. They are the query input `ChecksOf`, which what they change depends on.

use vun_command::{Args, Arity, Param, Values};
use vun_diag::Diagnostic;
use vun_sanitize::{Checks, ChecksOf};
use vun_sema::target::Variants;
use vun_target::Check;

use crate::Session;

pub const CHECKS: Param = Param::option("checks")
    .arity(Arity::Many)
    .about("what to check unsafe code for at run time, as `std.build.Options.checks` names it")
    .values(Values::Choice(|| Check::VARIANTS.iter().map(|(item, _)| item.name()).collect()));

/// Applies `--checks` to the session.
///
/// # Errors
/// Never: parsing checks each value names a check.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    let named = args.values(&CHECKS).filter_map(|name| {
        Check::VARIANTS.iter().find(|(item, _)| item.name() == name).map(|&(_, check)| check)
    });
    session.add_checks(named);
    Ok(())
}

impl Session {
    /// Adds `checks` to those the session's programs are built with.
    pub fn add_checks(&mut self, checks: impl IntoIterator<Item = Check>) {
        let current = self.db.get::<ChecksOf>(&());
        self.db.set::<ChecksOf>(&(), Checks::of(current.iter().chain(checks)));
    }

    /// The checks the session's programs are built with.
    #[must_use]
    pub fn checks(&self) -> Checks {
        self.db.get::<ChecksOf>(&())
    }
}
