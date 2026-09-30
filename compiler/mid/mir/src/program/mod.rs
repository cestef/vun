//! A program (spec 08, section 1): the functions its root file's exports reach. The exports are
//! the `#[link.export]` functions and data among the root file's bindings and the members of the
//! struct types they evaluate to; from each, every function a body calls or takes as a value, or
//! an exported datum holds, is reached, with the `$` arguments of its call.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::db::{FileRef, Parsed};
use vun_hir::tree::ExprKind;
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_query::{Cx, Cycle, Query};
use vun_sema::callable::Generic;
use vun_sema::decl::{DeclType, DeclValue};
use vun_sema::eval::Instantiate;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::{DeclRef, ExprRef};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use crate::{Concrete, Unready};

mod reach;

use reach::Reach;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Program {
    /// Every function reached, in the order found: the exports first.
    pub functions: Vec<Reached>,
    /// The exported functions, by their position in `functions`, and the symbol of each.
    pub exports: Vec<(usize, Symbol)>,
    /// The exported data, which no function makes: a vector table.
    pub data: Vec<ExportedDatum>,
    /// The declarations of the root file exporting a function or data, in source order.
    pub roots: Vec<DeclRef>,
    /// The declarations the linker provides that a body reaches.
    pub imports: Vec<DeclRef>,
    /// The functions reached whose bodies are not ready, and why.
    pub unready: Vec<(Instantiation, Unready)>,
    /// The exported declarations whose values have none at compile time.
    pub unevaluated: Vec<UnevaluatedExport>,
    /// The variants whose constructors are taken as values, each a function (`Body::constructor`),
    /// by its enum type and name.
    pub constructors: Vec<(Ty, Symbol)>,
}

/// A function of a program, and the statement that first reached it; `None` for a root.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Reached {
    pub i: Instantiation,
    pub by: Option<Statement>,
}

/// A statement of the function at `function` among a program's, which the program found first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Statement {
    pub function: usize,
    pub at: OriginId,
}

impl Program {
    /// The instantiations reached, in the order found.
    pub fn instantiations(&self) -> impl Iterator<Item = &Instantiation> {
        self.functions.iter().map(|reached| &reached.i)
    }

    /// The calls that led to `i` from outside generic code, innermost first: from each generic
    /// instantiation to the statement that reached it (spec 10, section 3). A function is always
    /// reached from one found before it, so the walk ends.
    #[must_use]
    pub fn leading(&self, cx: &Cx<'_>, i: &Instantiation) -> Vec<OriginId> {
        let mut calls = Vec::new();
        let mut reached = self.functions.iter().find(|r| r.i == *i);
        while let Some(Reached { i, by: Some(by) }) = reached
            && i.generic(cx)
        {
            calls.push(by.at);
            reached = self.functions.get(by.function);
        }
        calls
    }
}

/// A declaration whose value is exported as data.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct ExportedDatum {
    pub decl: DeclRef,
    pub value: Value,
    pub ty: Ty,
    pub symbol: Symbol,
    /// Its link section, when not the default one.
    pub section: Option<Symbol>,
}

/// An exported declaration whose value, or type, has none at compile time, and why.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct UnevaluatedExport {
    pub decl: DeclRef,
    pub symbol: Symbol,
    pub why: Unevaluated,
}

impl UnevaluatedExport {
    /// Its report: at the declaration, and where evaluation stopped in a function it calls.
    #[must_use]
    pub fn report(&self) -> Diagnostic {
        let msg = Msg::new()
            .text("the value exported as ")
            .code(self.symbol.as_str())
            .text(" cannot be computed at compile time: ")
            .text(self.why.why());
        let note = "an export is a function or data the program holds, so it is computed when compiling";
        let d = Diagnostic::error(errors::COMPTIME_EXPORT, msg).at(self.decl.decl.origin, None).note(note);
        match self.why {
            Unevaluated::Unbuilt(at) => d.label(at, Some(Msg::new().text("computing it stops here"))),
            _ => d,
        }
    }
}

/// Where the reach of a program starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Roots {
    /// The root file's exports: the program as built.
    #[default]
    Exports,
    /// The exports and every function of the root file that depends on no compile-time
    /// binding and runs at run time: what coverage measures.
    Functions,
    /// The exports among the members of a struct type, such as the program running a
    /// namespace's tests, `std.testing.runner(ns)`.
    Members(Ty),
}

/// The program whose root is a file, reached from its roots.
pub struct Programs;

impl Query for Programs {
    const NAME: &'static str = "programs";
    type Key = (FileRef, Roots);
    type Value = Arc<Program>;

    fn compute(cx: &Cx<'_>, (root, roots): &(FileRef, Roots)) -> Self::Value {
        let mut reach = Reach::new(cx);
        if let Roots::Members(ty) = roots {
            reach.members(*ty);
        } else if let Some(parsed) = cx.get::<Parsed>(root)
            && let Some(ns) = parsed.hir.root
        {
            let hir = &parsed.hir;
            for decl in hir.member_decls(ns) {
                let decl = DeclRef::new(root.clone(), hir, decl);
                let ty = cx.get::<DeclType>(&decl).map(|known| known.ty);
                let exported = |p: &Program| p.exports.len() + p.data.len();
                let before = exported(&reach.program);
                reach.export(&decl, cx.get::<DeclValue>(&decl), ty);
                if exported(&reach.program) > before {
                    reach.program.roots.push(decl);
                }
            }
            if *roots == Roots::Functions {
                // A closure is reached from the body that makes it, with its captures.
                let open = hir.open_functions();
                for (id, e) in hir.exprs.iter() {
                    if !matches!(e.kind, ExprKind::Fn { .. })
                        || open.contains(&id)
                        || hir.closure_of(id).is_some()
                    {
                        continue;
                    }
                    let f = Instantiation::plain(ExprRef::new(root.clone(), hir, id));
                    if !f.compile_time(cx) {
                        reach.function(f);
                    }
                }
            }
        }
        // The functions reached in turn, each wave's bodies made over threads: the ones the
        // wave reaches come after it, in the order a function at a time would find them.
        let mut next = 0;
        while next < reach.program.functions.len() {
            let wave: Vec<Instantiation> =
                reach.program.functions[next..].iter().map(|r| r.i.clone()).collect();
            next = reach.program.functions.len();
            for (i, body) in wave.iter().zip(cx.get_all::<Concrete>(&wave)) {
                reach.body(i, body);
            }
        }
        Arc::new(reach.program)
    }

    fn cycle(_: &Cx<'_>, _: &(FileRef, Roots), _: &Cycle) -> Self::Value {
        Arc::default()
    }
}
