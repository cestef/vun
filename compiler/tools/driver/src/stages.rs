//! The stages of the front end the driver runs so far, and the diagnostic groups each reports.
//!
//! Harnesses read this table to tell a missing diagnostic from one that belongs to a stage that
//! does not exist yet.

use vun_diag::{DiagCtxt, DiagId, errors};
use vun_hir::db::FileNames;
use vun_mir::{Programs, Roots};
use vun_mir_passes::borrows::BorrowChecks;
use vun_mir_passes::panics::PanicChecks;
use vun_mir_passes::places::PlaceStates;
use vun_origin::Resolve;
use vun_query::Query;
use vun_sema::check::Checked;
use vun_sema::typeck::Typed;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::trap::Unevaluated;
use vun_span::{FileId, SourceMap};
use vun_trace::{Activity, Timer, tracing};

use crate::Session;

/// Finding what a program reaches, as timed.
const REACH: Activity = Activity::stage("reach");
/// Reading the reports of typing each body a program reaches, as timed.
const BODIES: Activity = Activity::stage("bodies");

pub struct Stage {
    pub name: &'static str,
    pub reports: &'static [DiagId],
    pub input: Input,
    /// Runs it on a file loaded; `None` for loading itself.
    pub run: Option<fn(&mut Session, FileId)>,
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

/// Reading the sources.
pub const LOAD: Stage = Stage { name: "load", reports: &[errors::IO], input: Input::Source, run: None };

pub const STAGES: &[Stage] = &[
    LOAD,
    Stage {
        name: "syntax",
        reports: &[errors::SYNTAX],
        input: Input::Source,
        run: Some(|session, file| {
            session.parse_ahead(file);
            drop(session.parsed(file));
        }),
    },
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
        run: Some(Session::check_names),
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
        run: Some(Session::check_types),
    },
    Stage { name: "plan", reports: &[errors::PLAN], input: Input::Source, run: Some(Session::check_plan) },
    Stage {
        name: "places",
        reports: &[errors::PLACE],
        input: Input::Mir,
        run: Some(Session::check_bodies::<PlaceStates>),
    },
    Stage {
        name: "borrows",
        reports: &[errors::BORROW],
        input: Input::Mir,
        run: Some(Session::check_bodies::<BorrowChecks>),
    },
    Stage {
        name: "panics",
        reports: &[errors::PANIC],
        input: Input::Mir,
        run: Some(Session::check_bodies::<PanicChecks>),
    },
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

    /// Times the stage until the timer is dropped.
    pub fn time(&self) -> Timer {
        Activity::stage(self.name).start()
    }
}

impl Session {
    /// Runs every stage that exists on `file`.
    pub fn check(&mut self, file: FileId) {
        for stage in STAGES {
            if let Some(run) = stage.run {
                let _timer = stage.time();
                run(self, file);
                tracing::debug!(stage = stage.name, errors = self.diag.error_count(), "stage done");
            }
        }
        tracing::info!(file = %self.sources.file(file).path, errors = self.diag.error_count(), "checked");
    }

    /// Checks the types of `file`, as far as sema does yet.
    pub fn check_types(&mut self, file: FileId) {
        let key = self.file_ref(file);
        if let Some(reports) = self.db.get::<Checked>(&key) {
            self.diag.absorb(&reports);
        }
        // The bodies of other files its program compiles, a library's included: what is built
        // is checked, each typed where the program reaches it.
        let program = {
            let _timer = REACH.start();
            self.db.get::<Programs>(&(key.clone(), Roots::Functions))
        };
        let _timer = BODIES.start();
        for body in program.instantiations().filter(|i| i.f.literal.file != key) {
            if let Some(typed) = self.db.get::<Typed>(body) {
                self.diag.absorb(&typed.diag);
            }
        }
        // An exported value with none to place, and the names of the file where evaluating it
        // stopped, which the program runs at compile time though no body of it is reached.
        for export in &program.unevaluated {
            self.diag.error(export.report());
            if let Unevaluated::Unbuilt(at) = export.why {
                let file = self.file_ref(self.source_span(at).file);
                if file != key
                    && let Some(checked) = self.db.get::<FileNames>(&file)
                {
                    self.diag.absorb(&checked.diag);
                }
            }
        }
        // An error in a generic body, found through the root's calls and in the body itself.
        self.diag.reached_once();
    }

    /// The lines, first and last, of each function of `file` that MIR does not lower yet.
    pub fn unlowered(&mut self, file: FileId) -> Vec<(u32, u32)> {
        let bodies = self.bodies(file);
        let unlowered = bodies.into_iter().filter(|(_, body)| body.is_err());
        unlowered
            .map(|(f, _)| {
                let span = self.source_span(f.expr.origin);
                let source = self.sources.file(span.file);
                (source.line_col(span.range.lo).line, source.line_col(span.range.hi).line)
            })
            .collect()
    }

    /// Runs the MIR check `Q` on every function body of `file`, and on every function and
    /// instantiation its program reaches, wherever it is written: what the program compiles is
    /// checked, a library's bodies included, and a body checked only as written would miss what
    /// its arguments make of it.
    pub(crate) fn check_bodies<Q: Query<Key = Instantiation, Value = Option<DiagCtxt>>>(
        &mut self,
        file: FileId,
    ) {
        let plain = self.functions(file).into_iter().map(Instantiation::plain);
        let root = self.file_ref(file);
        let program = self.db.get::<Programs>(&(root, Roots::Functions));
        let mut seen = vun_hash::Set::default();
        let bodies: Vec<Instantiation> =
            plain.chain(program.instantiations().cloned()).filter(|body| seen.insert(body.clone())).collect();
        for body in bodies {
            if let Some(reports) = self.db.get::<Q>(&body) {
                self.diag.absorb(&reports);
            }
        }
    }
}
