//! The stages of the front end the driver runs so far, and the diagnostic groups each reports.
//!
//! Harnesses read this table to tell a missing diagnostic from one that belongs to a stage that
//! does not exist yet.

use vun_diag::{DiagCtxt, DiagId, errors};
use vun_query::Query;
use vun_span::{FileId, SourceMap};

use vun_mir_passes::borrows::BorrowChecks;
use vun_mir_passes::panics::PanicChecks;
use vun_mir_passes::places::PlaceStates;
use vun_sema::eval::Instantiation;

use crate::Session;

pub struct Stage {
    pub name: &'static str,
    pub reports: &'static [DiagId],
    pub input: Input,
}

/// What a stage reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// The source, through the front end.
    Source,
    /// Function bodies in MIR, which some forms do not lower to yet: a stage that reads MIR says
    /// nothing about a body that is not lowered.
    Mir,
}

pub const STAGES: &[Stage] = &[
    Stage { name: "load", reports: &[errors::IO], input: Input::Source },
    Stage { name: "syntax", reports: &[errors::SYNTAX], input: Input::Source },
    Stage {
        name: "names",
        reports: &[
            errors::NAMES_DUPLICATE,
            errors::NAMES_SHADOWED,
            errors::NAMES_UNRESOLVED,
            errors::BINDING_IRREFUTABLE_TEST,
            errors::META_ESCAPE,
            errors::PACKAGE,
        ],
        input: Input::Source,
    },
    Stage {
        name: "types",
        reports: &[
            errors::TYPES_MISMATCH,
            errors::TYPES_NO_MEMBER,
            errors::TYPES_NO_IMPL,
            errors::TYPES_CANNOT_INFER,
            errors::TYPES_ARITY,
            errors::INT_WIDTH_CAP,
            errors::NAMES_PRIVATE,
            errors::NAMES_VISIBILITY,
            errors::COMPTIME,
            errors::IMPL,
            errors::LAYOUT_BITS_WIDTH,
            errors::SAFETY,
            errors::BINDING_HOLE_VALUE,
            errors::COROUTINE,
            errors::META_FROZEN,
        ],
        input: Input::Source,
    },
    Stage { name: "plan", reports: &[errors::PLAN], input: Input::Source },
    Stage { name: "places", reports: &[errors::PLACE], input: Input::Mir },
    Stage { name: "borrows", reports: &[errors::BORROW], input: Input::Mir },
    Stage { name: "panics", reports: &[errors::PANIC], input: Input::Mir },
];

impl Stage {
    /// Whether some stage that exists reports `id`.
    #[must_use]
    pub fn implemented(id: &DiagId) -> bool {
        Self::of(id).is_some()
    }

    /// The stage that reports `id`, if one exists.
    #[must_use]
    pub fn of(id: &DiagId) -> Option<&'static Self> {
        STAGES.iter().find(|s| s.reports.iter().any(|group| id.within(group)))
    }
}

impl Session {
    /// Runs every stage that exists on `file`.
    pub fn check(&mut self, file: FileId) {
        self.parsed(file);
        self.check_names(file);
        self.check_types(file);
        self.check_plan(file);
        self.check_places(file);
        self.check_borrows(file);
        self.check_bodies::<PanicChecks>(file);
    }

    /// The lines, first and last, of each function of `file` that MIR does not lower yet.
    pub fn unlowered(&mut self, file: FileId) -> Vec<(u32, u32)> {
        let bodies = self.bodies(file);
        let unlowered = bodies.into_iter().filter(|(_, body)| body.is_err());
        unlowered
            .map(|(f, _)| {
                let span = self.origins.source_span(f.expr.origin);
                let source = self.sources.file(span.file);
                (source.line_col(span.range.lo).line, source.line_col(span.range.hi).line)
            })
            .collect()
    }

    /// Checks the place states of every function body of `file` (spec 05, section 2).
    pub fn check_places(&mut self, file: FileId) {
        self.check_bodies::<PlaceStates>(file);
    }

    /// Checks the borrow rules in every function body of `file` (spec 05, sections 4 and 8).
    pub fn check_borrows(&mut self, file: FileId) {
        self.check_bodies::<BorrowChecks>(file);
    }

    /// Runs the MIR check `Q` on every function body of `file`.
    fn check_bodies<Q: Query<Key = Instantiation, Value = Option<DiagCtxt>>>(&mut self, file: FileId) {
        for f in self.functions(file) {
            if let Some(reports) = self.db.get::<Q>(&Instantiation::plain(f)) {
                self.diag.absorb(&reports);
            }
        }
    }
}
