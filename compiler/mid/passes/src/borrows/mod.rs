//! Borrow rules (spec 05, sections 4 and 8), checked on MIR. Each `&mode place` is a loan, held
//! by the locals its value flows into; a scoped value built from others holds their loans, so a
//! call's scoped result is tied to every loan its arguments hold (section 4). A loan lives while a
//! local holding it is live, not to the end of a block. While it lives, an access to a place
//! overlapping its place conflicts when either is exclusive: the table `borrows` says which modes
//! are, a write is, a read is not. A loan of the function's own storage, a place not reached
//! through `.*`, that reaches the return escapes, and a value holding a borrow inside a type
//! marked `#[kind.free]` is reported where it is built. A `&temp` parameter's borrow is lent for
//! the call only: a call's result is not tied to what it is passed as one, and a callee's own
//! `&temp` parameter is a loan that escapes when it reaches the return.

mod access;
mod flow;
mod free;
mod later;

use std::collections::{BTreeMap, BTreeSet, HashSet};

use vun_codec::{Decode, Encode};
use vun_desugar::{Borrow, Extent};
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_grammar::TokenKind;
use vun_index::{IndexVec, index};
use vun_mir::{BlockId, Body, Local, Mir, Operand, Place, Projection, Rvalue};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema::env::Extents;
use vun_sema::kinds::{Kind, Kinds};
use vun_sema_ty::Value;
use vun_sema_ty::call::Instantiation;

index! {
    struct LoanId;
}

/// The borrow reports of an instantiation's body; `None` when it has no MIR yet.
pub struct BorrowChecks;

impl Query for BorrowChecks {
    const NAME: &'static str = "borrow_checks";
    type Key = Instantiation;
    type Value = Option<DiagCtxt>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        let body = cx.get::<Mir>(i).ok()?;
        let calls = Self::extents(cx, &body);
        let kinds: BTreeMap<Local, Kind> =
            body.locals.iter().filter_map(|(local, d)| Some((local, cx.get::<Kinds>(&d.ty?)))).collect();
        let having = |f: fn(&Kind) -> bool| kinds.iter().filter(|(_, k)| f(k)).map(|(&l, _)| l).collect();
        let locals = Locals {
            scoped: having(|k| k.is_scoped()),
            // A borrow stored in a free value is reported where its type is written: in the
            // function as written, or at the call instantiating it, not again in the instance.
            misplaced: if i.is_plain() { having(|k| k.misplaced) } else { BTreeSet::new() },
            linear: having(|k| k.is_linear()),
        };
        Some(Checker::new(&body, calls, locals).run())
    }
}

impl BorrowChecks {
    /// How long each call statement's callee holds each argument, where the callee is known.
    fn extents(cx: &Cx<'_>, body: &Body) -> BTreeMap<(BlockId, usize), Vec<Extent>> {
        let mut calls = BTreeMap::new();
        for (block, b) in body.blocks.iter() {
            for (index, stmt) in b.stmts.iter().enumerate() {
                let Rvalue::Call { callee, .. } = &stmt.value else { continue };
                let extents = match callee {
                    Operand::Const(Value::Fn(f)) => f.extents(cx),
                    Operand::Const(_) => None,
                    Operand::Place(p) => body.locals[p.local]
                        .ty
                        .and_then(|t| t.callable())
                        .map(|s| s.params.iter().map(|t| t.extent().unwrap_or(Extent::Tied)).collect()),
                };
                if let Some(extents) = extents {
                    calls.insert((block, index), extents);
                }
            }
        }
        calls
    }
}

/// Where a loan comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum Source {
    /// A borrow the body takes.
    Taken,
    /// A `&temp` parameter, lent for the call: it conflicts with nothing and must not escape.
    Lent,
}

/// A borrow taken in the body.
struct Loan {
    place: Place,
    mode: Borrow,
    source: Source,
    origin: OriginId,
}

/// The locals of each kind the checks treat apart.
struct Locals {
    /// Holding scoped values, which the loans of what they are built from flow into.
    scoped: BTreeSet<Local>,
    /// Whose type holds a scoped value inside a free one.
    misplaced: BTreeSet<Local>,
    /// Holding linear values, which a read of the whole moves.
    linear: BTreeSet<Local>,
}

struct Checker<'b> {
    body: &'b Body,
    loans: IndexVec<LoanId, Loan>,
    /// The loan each borrow statement takes, by its block and position.
    taken: BTreeMap<(BlockId, usize), LoanId>,
    /// The loans lent to the body by its `&temp` parameters, which hold them at the entry.
    lent: Vec<(Local, LoanId)>,
    /// How long each call's callee holds each argument, where it is known.
    calls: BTreeMap<(BlockId, usize), Vec<Extent>>,
    locals: Locals,
    diag: DiagCtxt,
    reported: HashSet<(DiagId, OriginId)>,
}

impl<'b> Checker<'b> {
    fn new(body: &'b Body, calls: BTreeMap<(BlockId, usize), Vec<Extent>>, locals: Locals) -> Self {
        let mut loans = IndexVec::new();
        let mut taken = BTreeMap::new();
        for (block, b) in body.blocks.iter() {
            for (index, stmt) in b.stmts.iter().enumerate() {
                let Rvalue::Ref { mode, place } = &stmt.value else { continue };
                let loan = loans.push(Loan {
                    place: place.clone(),
                    mode: *mode,
                    source: Source::Taken,
                    origin: stmt.origin,
                });
                taken.insert((block, index), loan);
            }
        }
        let mut lent = Vec::new();
        for &param in &body.params {
            let d = &body.locals[param];
            let Some((mode, ..)) = d.ty.and_then(|t| t.borrowing()) else { continue };
            if d.ty.and_then(|t| t.extent()) == Some(Extent::Temp) {
                let place = Place::of(param).project(Projection::Deref);
                lent.push((param, loans.push(Loan { place, mode, source: Source::Lent, origin: d.origin })));
            }
        }
        let diag = DiagCtxt::new();
        Self { body, loans, taken, lent, calls, locals, diag, reported: HashSet::new() }
    }

    fn run(mut self) -> DiagCtxt {
        self.free_values();
        // With no loan, nothing conflicts, escapes or is outlived.
        if self.loans.is_empty() {
            return self.diag;
        }
        let holding = self.holding();
        let live = self.liveness();
        for (block, holds) in holding.iter() {
            if let Some(holds) = holds {
                self.check(block, holds.clone(), &live);
            }
        }
        self.diag
    }

    /// Reports at `origin`, pointing at where `loan` was taken when given.
    fn report(&mut self, id: DiagId, origin: OriginId, msg: Msg, loan: Option<LoanId>, help: Option<Msg>) {
        if let Some(diagnostic) = self.reporting(id, origin, msg, loan, help) {
            self.diag.error(diagnostic);
        }
    }

    /// The report of `id` at `origin`, labeling where `loan` was taken, unless one was made.
    fn reporting(
        &mut self,
        id: DiagId,
        origin: OriginId,
        msg: Msg,
        loan: Option<LoanId>,
        help: Option<Msg>,
    ) -> Option<Diagnostic> {
        self.reported.insert((id, origin)).then(|| {
            let mut diagnostic = Diagnostic::error(id, msg).at(origin, None);
            if let Some(help) = help {
                diagnostic = diagnostic.child(ChildKind::Help, help);
            }
            if let Some(loan) = loan {
                let l = &self.loans[loan];
                let spelled = [TokenKind::Amp.text().unwrap_or_default(), &l.mode.to_string()].concat();
                let verb = match l.source {
                    Source::Taken => "borrowed as ",
                    Source::Lent => "lent as ",
                };
                let label = Msg::new().text(verb).code(spelled).text(" here");
                diagnostic = diagnostic.label(l.origin, Some(label));
            }
            diagnostic
        })
    }
}
