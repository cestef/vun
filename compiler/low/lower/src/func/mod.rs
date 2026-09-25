//! Lowering one function's concrete MIR: every local lives in a stack slot of its layout, read and
//! written through its address; each MIR block becomes a LIR block, and each statement the
//! instructions computing its value into its place. Values in SSA are transient, so a later pass
//! can promote slots to values.

mod bounds;
mod call;
mod constant;
mod convert;
mod coroutine;
mod float;
mod int;
mod memory;
mod narrow;
mod place;
mod primitive;
mod rvalue;
mod term;
mod trap;
mod variant;
mod verdict;

pub(crate) use coroutine::Mode;

use std::sync::Arc;
use vun_index::{Idx, IndexVec};
use vun_lir::{Builder, Function, Linkage, Scalar, Slot, Target, Terminator, Value};

use vun_mir::{BlockId, Body, Local, Operand, Role, Rvalue, Unready};
use vun_mir_passes::places::{Found, Placed};
use vun_origin::OriginId;
use vun_sema::eval::Instantiation;
use vun_sema::kinds::Kinds;
use vun_sema::primitives::abi::Abi;
use vun_sema::ty::Ty;

use crate::frame::FrameShape;
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
    /// The frame of the coroutine lowered, at this offset.
    Frame(u64),
}

pub(crate) struct FnLower<'l, 'a, 'db> {
    pub(crate) l: &'l mut Lowering<'a, 'db>,
    body: &'l Body,
    pub(crate) b: Builder,
    slots: IndexVec<Local, Storage>,
    blocks: IndexVec<BlockId, vun_lir::Block>,
    /// Where a result held in memory is written: the address the caller passed.
    out: Option<Value>,
    /// The address of the frame, in a coroutine's `resume` or `end`.
    frame: Option<Value>,
    /// What a coroutine's `resume` or `end` lowers the body as.
    coroutine: Option<coroutine::Coroutine>,
    /// Where each deferred statement at an exit goes, as the place states decide.
    defers: Arc<Found>,
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
        let cx = l.types.cx;
        let params = Self::runtime_params(cx, f, body);
        let defers = cx.get::<Placed>(f).unwrap_or_default();
        let mut function = Self::function(l, body, &params, f.f.abi(cx), linkage, defers)?;
        function.stack_max = f.f.attributes(cx).and_then(|a| a.stack_max());
        Ok(function)
    }

    /// `body` as a function taking `params` under `abi`, its deferred statements run as `defers`
    /// decide.
    pub(crate) fn function(
        l: &'l mut Lowering<'a, 'db>,
        body: &'l Body,
        params: &[Local],
        abi: Option<Abi>,
        linkage: Linkage,
        defers: Arc<Found>,
    ) -> Result<Function, Unlowered> {
        let at = body.origin;
        let name = l.types.cx.db().resource::<vun_origin::OriginTable>().declaration_name(at);
        let _span =
            vun_trace::tracing::debug_span!("lower", f = name.as_ref().map_or("", |n| n.as_str())).entered();
        if body.suspends() {
            return Err(Unlowered::unsupported(at, Form::Frame));
        }
        let param_types = params.iter().map(|&p| Self::typed(body, p, at)).collect::<Result<Vec<_>, _>>()?;
        let signature = l.signature(&param_types, Self::typed(body, body.ret, at)?, abi, at)?;
        let through = signature.through();
        let mut this = Self::new(l, defers, body, Builder::new(linkage, signature, at), at);
        this.storage(params, None)?;
        this.entry(params, through)?;
        this.create_blocks();
        let first = this.blocks[Idx::new(0)];
        this.b.terminate(Terminator::Jump(Target { block: first, args: Vec::new() }));
        this.lower_blocks()?;
        Ok(this.b.finish())
    }

    fn new(
        l: &'l mut Lowering<'a, 'db>,
        defers: Arc<Found>,
        body: &'l Body,
        b: Builder,
        at: OriginId,
    ) -> Self {
        Self {
            defers,
            b,
            l,
            body,
            slots: IndexVec::new(),
            blocks: IndexVec::new(),
            out: None,
            frame: None,
            coroutine: None,
            at,
        }
    }

    /// The parameters of `body` passed at run time: not `$` ones.
    fn runtime_params(cx: &vun_query::Cx<'_>, f: &Instantiation, body: &Body) -> Vec<Local> {
        let comptime = f.f.comptime(cx).unwrap_or_default();
        body.params
            .iter()
            .enumerate()
            .filter(|(i, _)| comptime.get(*i) != Some(&true))
            .map(|(_, &p)| p)
            .collect()
    }

    /// The type of `local` in `body`, which concrete MIR always has.
    fn typed(body: &Body, local: Local, at: OriginId) -> Result<Ty, Unlowered> {
        body.locals[local].ty.ok_or(Unlowered { at, why: Why::Unready(Unready::Untyped { at }) })
    }

    /// Where each local lives: nowhere when nothing reads or writes it or it has size 0, in the
    /// frame when `frame` holds it, in its data object when it is a top-level binding, else in a
    /// stack slot.
    fn storage(&mut self, params: &[Local], frame: Option<&FrameShape>) -> Result<(), Unlowered> {
        let body = self.body;
        let mut used = body.read();
        used.extend(params.iter().copied());
        used.extend(body.blocks.iter().flat_map(|(_, b)| b.stmts.iter().map(|s| s.place.local)));
        for (local, decl) in body.locals.iter() {
            if !used.contains(&local) {
                self.slots.push(Storage::None);
                continue;
            }
            let ty = Self::typed(body, local, self.at)?;
            if let Some(offset) = frame.and_then(|f| f.stored[local]) {
                self.slots.push(Storage::Frame(offset));
                continue;
            }
            let storage = match (self.l.types.repr(ty, decl.origin)?, &decl.role) {
                (Repr::Zero, _) => Storage::None,
                // Immutable storage holding a cell is written through it all the same.
                (_, Role::Static(global)) => {
                    let interior = self.l.types.cx.get::<Kinds>(&ty).interior;
                    Storage::Global(self.l.global(global, ty, interior, decl.origin)?)
                }
                (_, Role::StaticMut(global)) => {
                    Storage::Global(self.l.global(global, ty, true, decl.origin)?)
                }
                (Repr::Scalar(s), _) => {
                    let bytes = s.bytes(self.l.types.pointer_bits());
                    Storage::Slot(self.b.slot(bytes, bytes, decl.origin))
                }
                (Repr::Memory(layout), _) => {
                    Storage::Slot(self.b.slot(layout.size, layout.align, decl.origin))
                }
            };
            self.slots.push(storage);
        }
        Ok(())
    }

    /// A LIR block for each MIR block, after the entry.
    fn create_blocks(&mut self) {
        for (id, block) in self.body.blocks.iter() {
            let lowered = self.b.block(&[], block.term.origin);
            debug_assert_eq!(self.blocks.len(), id.index());
            self.blocks.push(lowered);
        }
    }

    /// Each MIR block's statements and terminator into its LIR block.
    fn lower_blocks(&mut self) -> Result<(), Unlowered> {
        let body = self.body;
        for (id, block) in body.blocks.iter() {
            self.b.switch_to(self.blocks[id]);
            for stmt in &block.stmts {
                self.at = stmt.origin;
                if stmt.value == Rvalue::Dead {
                    continue;
                }
                vun_trace::tracing::trace!(value = %stmt.value, "statement");
                let (dest, dest_ty) = self.place(&stmt.place)?;
                self.rvalue(&stmt.value, dest, dest_ty).inspect_err(|u| {
                    vun_trace::tracing::debug!(value = %stmt.value, why = %u, "unlowered");
                })?;
            }
            self.at = block.term.origin;
            self.term(id, &block.term.kind)?;
        }
        Ok(())
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
        Self::typed(self.body, local, self.at)
    }

    /// The address of `local`'s storage; `None` for a local of size 0.
    pub(crate) fn local(&mut self, local: Local) -> Option<Value> {
        match self.slots[local] {
            Storage::None => None,
            Storage::Slot(slot) => Some(self.b.slot_addr(slot, self.at)),
            Storage::Global(data) => Some(self.b.addr(vun_lir::Address::Data(data), self.at)),
            Storage::Frame(offset) => {
                let frame = self.frame?;
                Some(self.at_offset(frame, offset))
            }
        }
    }

    pub(crate) fn body(&self) -> &Body {
        self.body
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

    /// Stores `value` into `dest`, the place of what computed it, when it has one: the
    /// computation runs, for its effects and traps, whether its value is kept or not.
    pub(crate) fn keep(&mut self, value: Value, dest: Option<Value>) {
        if let Some(dest) = dest {
            self.b.store(value, dest, vun_lir::Access::Plain, self.at);
        }
    }

    /// The type of `operand`: a place's, or a constant's own.
    pub(crate) fn operand_ty(&self, operand: &Operand) -> Option<Ty> {
        match operand {
            Operand::Place(place) => place.ty(self.l.types.cx, &self.body.locals),
            Operand::Const(value) => value.ty(),
        }
    }
}
