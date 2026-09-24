//! Lowering one function's concrete MIR: every local lives in a stack slot of its layout, read and
//! written through its address; each MIR block becomes a LIR block, and each statement the
//! instructions computing its value into its place. Values in SSA are transient, so a later pass
//! can promote slots to values.

mod bounds;
mod call;
mod constant;
mod convert;
mod place;
mod primitive;
mod rvalue;
mod trap;

use vun_index::{Idx, IndexVec};
use vun_lir::{Builder, Function, Linkage, Scalar, Slot, Target, Terminator, Value};
use vun_mir::{BlockId, Body, Local, Operand, Role, Rvalue, TerminatorKind, Unready};
use vun_origin::OriginId;
use vun_sema::eval::Instantiation;
use vun_sema::ty::Ty;

use crate::program::Lowering;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered, Why};

/// Where a local lives.
#[derive(Clone, Copy)]
enum Storage {
    /// Nowhere: its type has size 0.
    None,
    Slot(Slot),
    /// The data object of the top-level binding it is the storage of.
    Global(vun_lir::DataId),
}

pub(crate) struct FnLower<'l, 'a, 'db> {
    pub(crate) l: &'l mut Lowering<'a, 'db>,
    body: &'l Body,
    pub(crate) b: Builder,
    slots: IndexVec<Local, Storage>,
    blocks: IndexVec<BlockId, vun_lir::Block>,
    /// Where a result held in memory is written: the address the caller passed.
    out: Option<Value>,
    /// The origin of what is being lowered.
    pub(crate) at: OriginId,
}

impl<'l, 'a, 'db> FnLower<'l, 'a, 'db> {
    pub(crate) fn lower(
        l: &'l mut Lowering<'a, 'db>,
        f: &Instantiation,
        body: &'l Body,
        linkage: Linkage,
    ) -> Result<Function, Unlowered> {
        let at = body.origin;
        if body.suspends() {
            return Err(Unlowered::unsupported(at, Form::Frame));
        }
        let cx = l.types.cx;
        let comptime = f.f.comptime(cx).unwrap_or_default();
        let ty = |local: Local| {
            body.locals[local].ty.ok_or(Unlowered { at, why: Why::Unready(Unready::Untyped { at }) })
        };
        let params: Vec<Local> = body
            .params
            .iter()
            .enumerate()
            .filter(|(i, _)| comptime.get(*i) != Some(&true))
            .map(|(_, &p)| p)
            .collect();
        let param_types = params.iter().map(|&p| ty(p)).collect::<Result<Vec<_>, _>>()?;
        let signature = l.signature(&param_types, ty(body.ret)?, f.f.abi(cx), at)?;
        let through = signature.through();
        let mut this = Self {
            b: Builder::new(linkage, signature, at),
            l,
            body,
            slots: IndexVec::new(),
            blocks: IndexVec::new(),
            out: None,
            at,
        };
        // A local nothing reads or writes needs no storage: a bound `$` parameter, or one holding
        // a constant that was propagated into its reads.
        let mut used = body.read();
        used.extend(params.iter().copied());
        used.extend(body.blocks.iter().flat_map(|(_, b)| b.stmts.iter().map(|s| s.place.local)));
        for (local, decl) in body.locals.iter() {
            if !used.contains(&local) {
                this.slots.push(Storage::None);
                continue;
            }
            let storage = match (this.l.types.repr(ty(local)?, decl.origin)?, &decl.role) {
                (Repr::Zero, _) => Storage::None,
                (_, Role::Static(global)) => {
                    Storage::Global(this.l.global(global, ty(local)?, false, decl.origin)?)
                }
                (_, Role::StaticMut(global)) => {
                    Storage::Global(this.l.global(global, ty(local)?, true, decl.origin)?)
                }
                (Repr::Scalar(s), _) => {
                    let bytes = s.bytes(this.l.types.pointer_bits());
                    Storage::Slot(this.b.slot(bytes, bytes, decl.origin))
                }
                (Repr::Memory(layout), _) => {
                    Storage::Slot(this.b.slot(layout.size, layout.align, decl.origin))
                }
            };
            this.slots.push(storage);
        }
        this.entry(&params, through)?;
        for (id, block) in body.blocks.iter() {
            let lowered = this.b.block(&[], block.term.origin);
            debug_assert_eq!(lowered.index(), id.index() + 1);
            this.blocks.push(lowered);
        }
        let first = this.blocks[Idx::new(0)];
        this.b.terminate(Terminator::Jump(Target { block: first, args: Vec::new() }));
        for (id, block) in body.blocks.iter() {
            this.b.switch_to(this.blocks[id]);
            for stmt in &block.stmts {
                this.at = stmt.origin;
                if stmt.value == Rvalue::Dead {
                    continue;
                }
                let (dest, dest_ty) = this.place(&stmt.place)?;
                this.rvalue(&stmt.value, dest, dest_ty)?;
            }
            this.at = block.term.origin;
            this.term(&block.term.kind)?;
        }
        Ok(this.b.finish())
    }

    /// The entry: each parameter stored into its local's slot, a value in memory copied from
    /// the address passed, and the address a result in memory is written through.
    fn entry(&mut self, params: &[Local], through: bool) -> Result<(), Unlowered> {
        let entry = self.b.current();
        let mut values = self.b.params(entry).to_vec().into_iter();
        for &param in params {
            let ty = self.ty(param)?;
            let Some(addr) = self.local(param) else { continue };
            let value = values.next().ok_or(Unlowered::unsupported(self.at, Form::Callee))?;
            match self.l.types.repr(ty, self.at)? {
                Repr::Scalar(_) => self.b.store(value, addr, vun_lir::Access::Plain, self.at),
                Repr::Memory(layout) => self.b.copy(addr, value, layout.size, self.at),
                Repr::Zero => {}
            }
        }
        if through {
            self.out = values.next();
        }
        Ok(())
    }

    /// The type of `local`, which concrete MIR always has.
    pub(crate) fn ty(&self, local: Local) -> Result<Ty, Unlowered> {
        self.body.locals[local]
            .ty
            .ok_or(Unlowered { at: self.at, why: Why::Unready(Unready::Untyped { at: self.at }) })
    }

    /// The address of `local`'s storage; `None` for a local of size 0.
    pub(crate) fn local(&mut self, local: Local) -> Option<Value> {
        match self.slots[local] {
            Storage::None => None,
            Storage::Slot(slot) => Some(self.b.slot_addr(slot, self.at)),
            Storage::Global(data) => Some(self.b.addr(vun_lir::Address::Data(data), self.at)),
        }
    }

    pub(crate) fn body(&self) -> &Body {
        self.body
    }

    fn term(&mut self, kind: &TerminatorKind) -> Result<(), Unlowered> {
        let jump = |block: vun_lir::Block| Target { block, args: Vec::new() };
        let term = match kind {
            TerminatorKind::Return => self.ret()?,
            TerminatorKind::Unreachable => Terminator::Unreachable,
            TerminatorKind::Goto(to) => Terminator::Jump(jump(self.blocks[*to])),
            TerminatorKind::Branch { cond, then, otherwise } => {
                let holds = self.holds(cond)?;
                Terminator::Branch {
                    cond: holds,
                    then: jump(self.blocks[*then]),
                    otherwise: jump(self.blocks[*otherwise]),
                }
            }
            // Only a deferred statement consuming nothing is known to run without the place states.
            TerminatorKind::Defer { consumes, run, .. } if consumes.is_empty() => {
                Terminator::Jump(jump(self.blocks[*run]))
            }
            TerminatorKind::Defer { .. } => return Err(Unlowered::unsupported(self.at, Form::Defer)),
            TerminatorKind::Yield { .. } | TerminatorKind::Ended => {
                return Err(Unlowered::unsupported(self.at, Form::Frame));
            }
        };
        self.b.terminate(term);
        Ok(())
    }

    /// Returns the result local: a scalar as the value, one in memory through the address the
    /// caller passed.
    fn ret(&mut self) -> Result<Terminator, Unlowered> {
        let ty = self.ty(self.body.ret)?;
        let Some(addr) = self.local(self.body.ret) else { return Ok(Terminator::Return(None)) };
        match (self.l.types.repr(ty, self.at)?, self.out) {
            (Repr::Scalar(s), _) => {
                Ok(Terminator::Return(Some(self.b.load(s, addr, vun_lir::Access::Plain, self.at))))
            }
            (Repr::Memory(layout), Some(out)) => {
                self.b.copy(out, addr, layout.size, self.at);
                Ok(Terminator::Return(None))
            }
            _ => Ok(Terminator::Return(None)),
        }
    }

    /// A new stack slot for a value of type `ty`, and its address; `None` for size 0.
    pub(crate) fn temp(&mut self, ty: Ty) -> Result<Option<Value>, Unlowered> {
        let size = |s: Scalar| s.bytes(self.l.types.pointer_bits());
        let (size, align) = match self.l.types.repr(ty, self.at)? {
            Repr::Zero => return Ok(None),
            Repr::Scalar(s) => (size(s), size(s)),
            Repr::Memory(layout) => (layout.size, layout.align),
        };
        let slot = self.b.slot(size, align, self.at);
        Ok(Some(self.b.slot_addr(slot, self.at)))
    }

    /// The type of `operand`: a place's, or a constant's own.
    pub(crate) fn operand_ty(&self, operand: &Operand) -> Option<Ty> {
        match operand {
            Operand::Place(place) => place.ty(self.l.types.cx, &self.body.locals),
            Operand::Const(value) => value.ty(),
        }
    }
}
