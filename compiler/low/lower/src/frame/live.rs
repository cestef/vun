//! Which locals a frame holds, and where (spec 06, section 5). A local is held when it is a
//! run-time parameter, when it is borrowed (a borrow may reach it across a `yield`), or when it is
//! live across a `yield`: read, after resuming or on the path ending the frame there, before it is
//! written whole. Two held locals share storage unless one is a parameter or borrowed, or both are
//! live across one `yield`. Each is placed, parameters first, then by local, at the lowest offset
//! past the state, aligned, overlapping none it may not share storage with.

use std::collections::BTreeSet;

use vun_codec::{Decode, Encode};
use vun_index::IndexVec;
use vun_lir::Scalar;
use vun_mir::{BlockId, Body, Local, Role, TerminatorKind};
use vun_sema::eval::Instantiation;
use vun_sema::layout::Layout;

use super::FrameShape;
use crate::repr::{Repr, Types};
use crate::unlowered::Unlowered;

/// The locals live across each `yield` of a body.
struct Suspensions {
    points: Vec<BlockId>,
    /// By point, the locals live across it.
    across: Vec<BTreeSet<Local>>,
}

impl Suspensions {
    /// By backward dataflow: a local is live into a block when the block reads it before
    /// writing it whole, or it is live out of the block and the block does not write it whole.
    fn of(body: &Body) -> Self {
        let count = body.blocks.len();
        let mut used: IndexVec<BlockId, BTreeSet<Local>> = IndexVec::filled(BTreeSet::new(), count);
        let mut killed: IndexVec<BlockId, BTreeSet<Local>> = IndexVec::filled(BTreeSet::new(), count);
        for (id, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                used[id].extend(stmt.reads().into_iter().filter(|l| !killed[id].contains(l)));
                killed[id].extend(stmt.writes_whole().filter(|l| !used[id].contains(l)));
            }
            used[id].extend(block.term.kind.reads().into_iter().filter(|l| !killed[id].contains(l)));
        }
        let mut live: IndexVec<BlockId, BTreeSet<Local>> = IndexVec::filled(BTreeSet::new(), count);
        let mut changed = true;
        while changed {
            changed = false;
            for (id, block) in body.blocks.iter().collect::<Vec<_>>().into_iter().rev() {
                let out: BTreeSet<Local> =
                    block.term.kind.successors().iter().flat_map(|s| live[*s].clone()).collect();
                let into: BTreeSet<Local> =
                    used[id].iter().copied().chain(out.difference(&killed[id]).copied()).collect();
                if into != live[id] {
                    live[id] = into;
                    changed = true;
                }
            }
        }
        let (mut points, mut across) = (Vec::new(), Vec::new());
        for (id, block) in body.blocks.iter() {
            if let TerminatorKind::Yield { resume, cancel, .. } = &block.term.kind {
                points.push(id);
                across.push(live[*resume].union(&live[*cancel]).copied().collect());
            }
        }
        Self { points, across }
    }

    /// Whether `a` and `b` are live across one `yield`.
    fn meet(&self, a: Local, b: Local) -> bool {
        self.across.iter().any(|live| live.contains(&a) && live.contains(&b))
    }
}

/// A local placed in a frame: where it starts and ends, and whether it shares storage with none.
#[derive(Clone, Copy, Encode, Decode)]
struct Placed {
    local: Local,
    start: u64,
    end: u64,
    alone: bool,
}

impl FrameShape {
    pub(crate) fn of(
        types: Types<'_, '_>,
        instantiation: &Instantiation,
        body: &Body,
    ) -> Result<Self, Unlowered> {
        let suspensions = Suspensions::of(body);
        let comptime = instantiation.f.comptime(types.cx).unwrap_or_default();
        let params: Vec<Local> = body
            .params
            .iter()
            .enumerate()
            .filter(|(i, _)| comptime.get(*i) != Some(&true))
            .map(|(_, &p)| p)
            .collect();
        let borrowed: BTreeSet<Local> =
            body.blocks.iter().flat_map(|(_, b)| b.stmts.iter().filter_map(vun_mir::Stmt::borrows)).collect();
        let across: BTreeSet<Local> = suspensions.across.iter().flatten().copied().collect();
        let held = params.iter().copied().chain(
            body.locals
                .iter()
                .map(|(l, _)| l)
                .filter(|l| !params.contains(l) && (borrowed.contains(l) || across.contains(l))),
        );
        let state = Self::state(suspensions.points.len());
        let state_bytes = state.bytes(types.pointer_bits());
        let (mut placed, mut align) = (Vec::<Placed>::new(), state_bytes);
        for local in held {
            let decl = &body.locals[local];
            if matches!(decl.role, Role::Static(_) | Role::StaticMut(_)) {
                continue;
            }
            let Some(ty) = decl.ty else { continue };
            let (size, local_align) = match types.repr(ty, decl.origin)? {
                Repr::Zero => continue,
                Repr::Scalar(s) => (s.bytes(types.pointer_bits()), s.bytes(types.pointer_bits())),
                Repr::Memory(layout) => (layout.size, layout.align),
            };
            let alone = params.contains(&local) || borrowed.contains(&local);
            let clashes = |other: &Placed| alone || other.alone || suspensions.meet(local, other.local);
            let mut start = state_bytes.next_multiple_of(local_align);
            while let Some(other) =
                placed.iter().find(|p| clashes(p) && p.start < start + size && start < p.end)
            {
                start = other.end.next_multiple_of(local_align);
            }
            placed.push(Placed { local, start, end: start + size, alone });
            align = align.max(local_align);
        }
        let end = placed.iter().map(|p| p.end).max().unwrap_or(state_bytes);
        let mut stored = IndexVec::filled(None, body.locals.len());
        for p in &placed {
            stored[p.local] = Some(p.start);
        }
        let layout = Layout { size: end.next_multiple_of(align), align, niche: None };
        Ok(Self { state, layout, stored, points: suspensions.points })
    }

    /// The narrowest integer telling apart the states of a frame of `points` suspension points:
    /// not started, suspended at each, done.
    fn state(points: usize) -> Scalar {
        let states = points as u128 + 2;
        Scalar::INTEGERS.into_iter().find(|s| states <= 1u128 << s.bits(0).min(127)).unwrap_or(Scalar::I32)
    }
}
