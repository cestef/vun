//! Lowering a program (spec 08, section 1): each function its exports reach, each coroutine's
//! `end` after them and each constructor taken as a value is lowered on its own (`piece`), over
//! threads, and placed in one unit (`assemble`).

use std::sync::Arc;

use vun_hir::db::FileRef;
use vun_lir::Unit;
use vun_mir::{Program, Programs, Roots, Unready};
use vun_query::{Cx, Query};
use vun_sema_ty::call::Instantiation;
use vun_trace::Activity;

use crate::assemble::Assembly;
use crate::inlined::{InlinedLir, Inlining};
use crate::lowering::Lowering;
use crate::piece::{FunctionLir, Piece};
use crate::unlowered::{Unlowered, Why};

/// Numbering a program's functions, as timed: its root file's environment, which functions
/// suspend and which the checks run in.
const NUMBER: Activity = Activity::stage("number functions");
/// Placing each function's ids in the program's unit, as timed.
const PLACE: Activity = Activity::stage("place functions");

/// The LIR of the program whose root is a file, reached from its roots.
pub struct Lowered;

impl Query for Lowered {
    const NAME: &'static str = "lowered";
    type Key = (FileRef, Roots);
    type Value = Arc<Result<Unit, Unlowered>>;

    fn compute(cx: &Cx<'_>, (root, roots): &Self::Key) -> Self::Value {
        Arc::new(Lowered::program(cx, root, *roots))
    }
}

impl Lowered {
    fn program(cx: &Cx<'_>, root: &FileRef, roots: Roots) -> Result<Unit, Unlowered> {
        let program = cx.get::<Programs>(&(root.clone(), roots));
        let numbering = NUMBER.start();
        let l = Lowering::new(cx, root, true)?;
        if let Some((i, unready)) = program.unready.first() {
            let unlowered = Unlowered::new(i.f.literal.expr.origin, Why::Unready(unready.clone()));
            return Err(unlowered.through(program.leading(cx, i)));
        }
        if let Some(export) = program.unevaluated.first() {
            let at = export.decl.decl.origin;
            let unready = Unready::Unevaluated { at, why: export.why.clone() };
            return Err(Unlowered::new(at, Why::Unready(unready)));
        }
        let at = Lowering::root_of(&l.env);
        let mut assembly = Assembly::new(&l);
        let unchecked = Lowering::unchecked(cx, root, &program);
        let pieces = assembly.number(&program, &unchecked, at)?;
        drop(numbering);
        let keys: Vec<Inlining> =
            pieces.into_iter().map(|piece| Inlining { piece, unchecked: unchecked.clone() }).collect();
        let lirs = cx.get_all::<InlinedLir>(&keys);
        let _placing = PLACE.start();
        if let Some(failure) = Self::failure(cx, &program, &keys, &lirs) {
            return Err(failure);
        }
        for lir in lirs.iter().filter_map(|lir| lir.as_ref().as_ref().ok()) {
            assembly.add(lir, at)?;
        }
        // Each exported datum encoded on its own, the ids it names placed as a function's are.
        for datum in &program.data {
            let mut own = Lowering::new(cx, root, true)?;
            let origin = datum.decl.decl.origin;
            let bytes = own.encode(&datum.value, datum.ty, origin)?;
            let align = own.types.shape(datum.ty, origin)?.layout.align;
            assembly.export(&own.refs, &bytes, align, (datum.symbol, datum.section), origin)?;
        }
        Ok(assembly.finish())
    }

    /// Why the pieces `keys` of `program`, lowered as `lirs`, make no program, if they do not: a
    /// library's report, which says why in its own words, else the first failure; traced through
    /// the calls that led to it.
    fn failure(
        cx: &Cx<'_>,
        program: &Program,
        keys: &[Inlining],
        lirs: &[<InlinedLir as Query>::Value],
    ) -> Option<Unlowered> {
        let failed =
            keys.iter().zip(lirs).filter_map(|(key, lir)| Some((&key.piece, lir.as_ref().as_ref().err()?)));
        let (piece, unlowered) =
            failed.min_by_key(|(_, unlowered)| !matches!(unlowered.why, Why::Reported(_)))?;
        let calls = piece.instantiation().map(|i| program.leading(cx, i)).unwrap_or_default();
        Some(unlowered.clone().through(calls))
    }
}

/// Each function a file's functions reach (`Roots::Functions`), lowered on its own, and why it
/// does not lower if it does not: what coverage measures.
pub struct EachLowered;

impl Query for EachLowered {
    const NAME: &'static str = "each_lowered";
    type Key = FileRef;
    type Value = Arc<Vec<(Instantiation, Result<(), Unlowered>)>>;

    fn compute(cx: &Cx<'_>, root: &FileRef) -> Self::Value {
        let program = cx.get::<Programs>(&(root.clone(), Roots::Functions));
        let unchecked = Lowering::unchecked(cx, root, &program);
        let pieces: Vec<Piece> = program.instantiations().map(|f| Piece::of(f, false, &unchecked)).collect();
        let lowered = cx.get_all::<FunctionLir>(&pieces);
        let each = program.instantiations().zip(lowered.iter());
        let each = each.map(|(f, lir)| (f.clone(), lir.as_ref().as_ref().map(drop).map_err(Clone::clone)));
        Arc::new(each.collect())
    }
}
