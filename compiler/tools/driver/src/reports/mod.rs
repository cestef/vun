//! What `vun check` measures beyond faults, each asked for by an option (spec 03, section 8;
//! spec 04, section 1): the options and the measures are the variants of [`Report`].

use vun_command::{Args, Param};
use vun_span::FileId;
use vun_term::Shell;
use vun_variants::Variants;

use crate::Session;

mod generics;
mod widths;

pub use generics::Generic;
pub use widths::WideRow;

/// A measure of a checked program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Report {
    /// The arithmetic intermediates wider than the target's register.
    Widths,
    /// The instantiations and the code of each generic function.
    Generics,
}

impl Report {
    /// The option asking for it.
    #[must_use]
    pub const fn param(self) -> Param {
        match self {
            Self::Widths => WIDEN,
            Self::Generics => META_STATS,
        }
    }
}

const WIDEN: Param =
    Param::flag("widen").about("list the arithmetic intermediates wider than the target's register");
const META_STATS: Param = Param::flag("meta-stats")
    .about("report the instantiations and the code size of each generic function the program reaches");

/// The measures asked for, and what they found.
#[derive(Default)]
pub struct Reports {
    wanted: Vec<Report>,
    pub widths: Vec<WideRow>,
    pub generics: Vec<Generic>,
}

impl Reports {
    /// The measures `args` ask for.
    #[must_use]
    pub fn of(args: &Args) -> Self {
        Self::asking(Report::ALL.iter().copied().filter(|r| args.flag(&r.param())))
    }

    /// The measures `wanted`.
    #[must_use]
    pub fn asking(wanted: impl IntoIterator<Item = Report>) -> Self {
        Self { wanted: wanted.into_iter().collect(), widths: Vec::new(), generics: Vec::new() }
    }

    fn wants(&self, report: Report) -> bool {
        self.wanted.contains(&report)
    }
}

impl Session {
    /// Asks for the measures `args` name.
    pub fn measure(&mut self, args: &Args) {
        self.reports = Reports::of(args);
    }

    /// Measures `file` as asked, after it is checked.
    pub fn measured(&mut self, file: FileId) {
        if self.reports.wants(Report::Widths) {
            let widths = self.widths(file);
            self.reports.widths.extend(widths);
        }
        if self.reports.wants(Report::Generics) {
            let generics = self.generics(file);
            self.reports.generics.extend(generics);
        }
    }

    /// Shows what the measures found.
    ///
    /// # Errors
    /// Fails when the shell cannot be written.
    pub fn show_reports(&mut self, shell: &mut Shell<'_>) -> std::io::Result<()> {
        if self.reports.wants(Report::Widths) {
            shell.table(&std::mem::take(&mut self.reports.widths))?;
        }
        let mut generics = std::mem::take(&mut self.reports.generics);
        generics.sort_by(|a, b| b.statements.cmp(&a.statements).then_with(|| a.label().cmp(&b.label())));
        if self.reports.wants(Report::Generics) {
            shell.table(&generics)?;
        }
        Ok(())
    }
}
