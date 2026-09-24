//! Borrow rules (spec 05, sections 4 and 8), checked on MIR. Each `&mode place` is a loan, held
//! by the locals its value flows into; a scoped value built from others holds their loans, so a
//! call's scoped result is tied to every loan its arguments hold (section 4). A loan lives while a
//! local holding it is live, not to the end of a block. While it lives, an access to a place
//! overlapping its place conflicts when either is exclusive: the table `borrows` says which modes
//! are, a write is, a read is not. A loan of the function's own storage, a place not reached
//! through `.*`, that reaches the return escapes, and a value holding a borrow inside a type
//! marked `#[kind.free]` is reported where it is built.

mod access;
mod flow;
mod free;

use std::collections::{BTreeMap, BTreeSet};

use overt_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use overt_grammar::{Access, BORROWS};
use overt_index::{IndexVec, index};
use overt_intern::Symbol;
use overt_mir::{BlockId, Body, Local, Mir, Place, Rvalue};
use overt_origin::OriginId;
use overt_query::{Cx, Query};
use overt_sema::eval::Instantiation;
use overt_sema::kinds::{Kind, Kinds};

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
        let kinds: BTreeMap<Local, Kind> =
            body.locals.iter().filter_map(|(local, d)| Some((local, cx.get::<Kinds>(&d.ty?)))).collect();
        let having = |f: fn(&Kind) -> bool| kinds.iter().filter(|(_, k)| f(k)).map(|(&l, _)| l).collect();
        Some(Checker::new(&body, having(|k| k.is_scoped()), having(|k| k.misplaced)).run())
    }
}

/// A borrow taken in the body.
struct Loan {
    place: Place,
    access: Access,
    origin: OriginId,
}

struct Checker<'b> {
    body: &'b Body,
    loans: IndexVec<LoanId, Loan>,
    /// The loan each borrow statement takes, by its block and position.
    taken: BTreeMap<(BlockId, usize), LoanId>,
    /// The locals holding scoped values, which the loans of what they are built from flow into.
    scoped: BTreeSet<Local>,
    /// The locals whose type holds a scoped value inside a free one.
    misplaced: BTreeSet<Local>,
    diag: DiagCtxt,
    reported: BTreeSet<(DiagId, OriginId)>,
}

impl<'b> Checker<'b> {
    fn new(body: &'b Body, scoped: BTreeSet<Local>, misplaced: BTreeSet<Local>) -> Self {
        let mut loans = IndexVec::new();
        let mut taken = BTreeMap::new();
        for (block, b) in body.blocks.iter() {
            for (index, stmt) in b.stmts.iter().enumerate() {
                let Rvalue::Ref { mode, place } = &stmt.value else { continue };
                let loan = loans.push(Loan {
                    place: place.clone(),
                    access: Self::access(*mode),
                    origin: stmt.origin,
                });
                taken.insert((block, index), loan);
            }
        }
        Self { body, loans, taken, scoped, misplaced, diag: DiagCtxt::new(), reported: BTreeSet::new() }
    }

    fn run(mut self) -> DiagCtxt {
        let holding = self.holding();
        let live = self.liveness();
        self.free_values();
        for (block, holds) in holding.iter() {
            if let Some(holds) = holds {
                self.check(block, holds.clone(), &live);
            }
        }
        self.diag
    }

    /// What others may do with a place borrowed in `mode` (the table `borrows`).
    fn access(mode: Option<Symbol>) -> Access {
        let word = mode.map_or("", Symbol::as_str);
        BORROWS.iter().find(|(m, ..)| *m == word).map_or(Access::Shared, |&(.., access)| access)
    }

    /// Reports at `origin`, pointing at where `loan` was taken when given.
    fn report(&mut self, id: DiagId, origin: OriginId, msg: Msg, loan: Option<LoanId>) {
        if self.reported.insert((id.clone(), origin)) {
            let mut diagnostic = Diagnostic::error(id, msg).at(origin, None);
            if let Some(loan) = loan {
                diagnostic =
                    diagnostic.label(self.loans[loan].origin, Some(Msg::new().text("borrowed here")));
            }
            self.diag.error(diagnostic);
        }
    }
}
