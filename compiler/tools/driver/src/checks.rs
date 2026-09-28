//! The checks of unsafe code at run time a session's programs are built with (spec 07, section
//! 7): `--checks` names entries of `lang.check.Kind`, and a program of a plan adds those of its
//! `Options.checks`. They are the query input `ChecksOf`, which what they change depends on.

use vun_command::{Args, Arity, Param, Values};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_lir::Unit;
use vun_query::Sharing;
use vun_sanitize::Operations;
use vun_sema::checks::ChecksOf;
use vun_sema::target::Variants;
use vun_span::FileId;
use vun_target::Check;
use vun_target::Checks;

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
        // Shared memos are kept by the checks, which they may read.
        self.db.set_as::<ChecksOf>(&(), Checks::of(current.iter().chain(checks)), Sharing::Chosen);
    }

    /// The checks the session's programs are built with.
    #[must_use]
    pub fn checks(&self) -> Checks {
        self.db.get::<ChecksOf>(&())
    }

    /// Reports `check.unsupported` when the program rooted at `file`, built with checks, still
    /// imports an operation of `lang.check`: its runtime does not export it.
    pub(crate) fn provided(&mut self, file: FileId, unit: &Unit) -> Result<(), ErrorGuaranteed> {
        let checks = self.checks();
        if checks.is_empty() {
            return Ok(());
        }
        let package = self.file_ref(file).package;
        let operations = self.db.get::<Operations>(&package);
        let imported = unit.imports.iter().find_map(|(_, import)| {
            operations
                .iter()
                .find(|(_, symbol)| *symbol == import.symbol)
                .map(|&(item, _)| (item, import.origin))
        });
        let Some((item, at)) = imported else { return Ok(()) };
        let mut msg = Msg::new()
            .text("the runtime of the program does not provide ")
            .code(item.to_string())
            .text(", which its checks need:");
        for check in checks.iter() {
            msg = msg.text(" ").code(check.item().name());
        }
        Err(self.diag.error(Diagnostic::error(errors::CHECK_UNSUPPORTED, msg).at(at, None)))
    }
}
