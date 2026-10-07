//! Lowering one function's concrete MIR: every local lives in a stack slot of its layout, read and
//! written through its address; each MIR block becomes a LIR block, and each statement the
//! instructions computing its value into its place. Values in SSA are transient, so a later pass
//! can promote slots to values.

mod asm;
mod bounds;
mod call;
mod constant;
mod convert;
mod coroutine;
mod cover;
mod float;
mod int;
mod memory;
mod narrow;
mod place;
mod primitive;
mod redzone;
mod rounded;
mod rvalue;
mod stack;
mod storage;
mod term;
mod trap;
mod variant;
mod verdict;

use std::sync::Arc;

pub(crate) use coroutine::Mode;
use storage::Storage;
use vun_index::{Idx, IndexVec};
use vun_lir::{Builder, Conv, Function, Linkage, Scalar, Slot, Target, Terminator, Value};
use vun_mir::{BlockId, Body, Local, Operand, Rvalue, Unready};
use vun_mir_passes::places::{Found, Placed};
use vun_origin::OriginId;
use vun_primitives::abi::Abi;
use vun_sema::attributes::Attributed;
use vun_sema::callable::Parameters;
use vun_sema::link::Exported;
use vun_sema_ty::Ty;
use vun_sema_ty::call::Instantiation;

use crate::lowering::Lowering;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered, Why};

pub(crate) struct FnLower<'l, 'a, 'db> {
    pub(crate) l: &'l mut Lowering<'a, 'db>,
    body: &'l Body,
    pub(crate) b: Builder,
    slots: IndexVec<Local, Storage>,
    /// Whether the checks of unsafe code apply in the function, and the slots they follow by a
    /// redzone, each with the bytes of its local (`redzone`).
    checked: bool,
    guards: Vec<(Slot, u64)>,
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
        let checked = l.checked;
        let mut function = Self::function(l, body, &params, f.f.abi(cx), linkage, defers, checked)?;
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
        checked: bool,
    ) -> Result<Function, Unlowered> {
        let at = body.origin;
        let name = vun_origin::OriginTable::global().declaration_name(at);
        let _span =
            vun_trace::tracing::debug_span!("lower", f = name.as_ref().map_or("", |n| n.as_str())).entered();
        if body.suspends() {
            return Err(Unlowered::unsupported(at, Form::Frame));
        }
        let param_types = params.iter().map(|&p| Self::typed(body, p, at)).collect::<Result<Vec<_>, _>>()?;
        let signature = l.signature(&param_types, Self::typed(body, body.ret, at)?, abi, at)?;
        let through = signature.through();
        // A naked function has no frame to check through.
        let naked = signature.conv == Conv::Naked;
        let mut this = Self::new(l, defers, body, Builder::new(linkage, signature, at), at);
        this.checked = checked && !naked;
        this.storage(params, None)?;
        this.entry(params, through)?;
        this.guard()?;
        this.stack_check()?;
        this.create_blocks();
        let first = this.blocks[Idx::new(0)];
        this.b.terminate(Terminator::Jump(Target { block: first, args: Vec::new() }));
        this.lower_blocks()?;
        let function = this.b.finish();
        vun_trace::tracing::debug!(blocks = function.blocks.len(), insts = function.insts.len(), "lowered");
        Ok(function)
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
            checked: false,
            guards: Vec::new(),
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
        body.locals[local].ty.ok_or(Unlowered::new(at, Why::Unready(Unready::Untyped { at, said: None })))
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
            self.at = block.term.origin;
            self.count(id)?;
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
            self.term(id, &block.term.kind).inspect_err(|u| {
                vun_trace::tracing::debug!(block = id.index(), why = %u, "a terminator unlowered");
            })?;
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
            Operand::Const(value) => value.ty().or_else(|| self.l.env.value_type(value).ok()),
        }
    }
}
