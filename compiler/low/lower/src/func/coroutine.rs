//! Coroutine functions (spec 06): one body lowered twice. `resume` goes, by the frame's state, to
//! the start or to the block after the `yield` the frame is suspended at, runs to the next
//! `yield` or to the return, and gives the step, `yielded` or `done`; a frame done traps. `end`
//! goes to the path ending the frame at the `yield` it is suspended at, which runs what is
//! pending there; a frame not started or done has nothing pending. Callers construct a frame in
//! place, its state and its parameters, and call `resume` and `end` with its address.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_index::Idx;
use vun_items::Item;
use vun_lir::{Access, Builder, Callee, Function, InstKind, Linkage, Target, Terminator, Value};
use vun_mir::{Aggregate, BlockId, Body, Operand, Place, TerminatorKind};
use vun_sema::eval::Instantiation;
use vun_sema::ty::{Ty, TyKind};

use super::FnLower;
use crate::frame::{FrameShape, FrameState, Frames};
use crate::lowering::Lowering;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered};

/// What a coroutine's body is lowered as.
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub(crate) enum Mode {
    Resume,
    End,
}

pub(crate) struct Coroutine {
    shape: Arc<FrameShape>,
    /// What `resume` gives: `lang.Step(Y, R)`.
    step: Ty,
    mode: Mode,
}

impl Lowering<'_, '_> {
    /// The shape of frames of type `frame`.
    pub(crate) fn frame_shape(&self, frame: Ty) -> Result<Arc<FrameShape>, Unlowered> {
        let shaped = self.types.cx.get::<Frames>(&frame);
        match &*shaped {
            Ok(shape) => Ok(Arc::new(shape.clone())),
            Err(unlowered) => Err(unlowered.clone()),
        }
    }
}

impl<'l, 'a, 'db> FnLower<'l, 'a, 'db> {
    /// The `resume` or `end` of the coroutine instantiation `f`, whose body is `body`.
    pub(crate) fn coroutine(
        l: &'l mut Lowering<'a, 'db>,
        f: &Instantiation,
        body: &'l Body,
        mode: Mode,
    ) -> Result<Function, Unlowered> {
        let at = body.origin;
        let unsupported = || Unlowered::unsupported(at, Form::Frame);
        let frame = f.signature(l.types.cx).map_err(|_| unsupported())?.result;
        if !matches!(&*frame, TyKind::Frame(_)) {
            return Err(unsupported());
        }
        let shape = l.frame_shape(frame)?;
        let step = l.env.resumed(frame).ok_or_else(unsupported)?;
        let signature = l.frame_signature((mode == Mode::Resume).then_some(step), at)?;
        let through = signature.through();
        let defers = l.types.cx.get::<vun_mir_passes::places::Placed>(f).unwrap_or_default();
        let mut this = Self::new(l, defers, body, Builder::new(Linkage::Internal, signature, at), at);
        let values = this.b.params(this.b.current()).to_vec();
        this.frame = values.first().copied();
        if through {
            this.out = values.get(1).copied();
        }
        let params = Self::runtime_params(this.l.types.cx, f, body);
        this.storage(&params, Some(&shape))?;
        this.create_blocks();
        let targets: Vec<(u128, BlockId)> = match mode {
            Mode::Resume => std::iter::once((shape.stored(FrameState::Start), BlockId::new(0)))
                .chain(shape.points.iter().enumerate().filter_map(|(k, &b)| {
                    Some((shape.stored(FrameState::Suspended(k)), Self::after(body, b, Mode::Resume)?))
                }))
                .collect(),
            Mode::End => shape
                .points
                .iter()
                .enumerate()
                .filter_map(|(k, &b)| {
                    Some((shape.stored(FrameState::Suspended(k)), Self::after(body, b, Mode::End)?))
                })
                .collect(),
        };
        let frame_at = this.frame.ok_or_else(unsupported)?;
        let state = this.b.load(shape.state, frame_at, Access::Plain, at);
        let cases = targets
            .into_iter()
            .map(|(s, b)| (s, Target { block: this.blocks[b], args: Vec::new() }))
            .collect();
        let otherwise = this.b.block(&[], at);
        this.b.terminate(Terminator::Switch {
            value: state,
            cases,
            default: Target { block: otherwise, args: Vec::new() },
        });
        this.b.switch_to(otherwise);
        match mode {
            Mode::Resume => this.trap()?,
            Mode::End => this.b.terminate(Terminator::Return(None)),
        }
        this.coroutine = Some(Coroutine { shape, step, mode });
        this.lower_blocks()?;
        Ok(this.b.finish())
    }

    /// Where a frame suspended at the `yield` ending `block` goes on: after it when resumed, on
    /// its cancel path when ended.
    fn after(body: &Body, block: BlockId, mode: Mode) -> Option<BlockId> {
        match &body.blocks[block].term.kind {
            TerminatorKind::Yield { resume, cancel, .. } => {
                Some(if mode == Mode::Resume { *resume } else { *cancel })
            }
            _ => None,
        }
    }

    /// How a coroutine's body ends block `id` when its terminator suspends or finishes: `None`
    /// for any other terminator.
    pub(super) fn suspension(
        &mut self,
        id: BlockId,
        kind: &TerminatorKind,
    ) -> Result<Option<Terminator>, Unlowered> {
        let Some(c) = &self.coroutine else { return Ok(None) };
        let (mode, shape) = (c.mode, Arc::clone(&c.shape));
        let row = vun_desugar::ITERATION.first().ok_or(Unlowered::unsupported(self.at, Form::Frame))?;
        Ok(Some(match (kind, mode) {
            (TerminatorKind::Yield { value, .. }, Mode::Resume) => {
                let k = shape
                    .points
                    .iter()
                    .position(|&p| p == id)
                    .ok_or(Unlowered::unsupported(self.at, Form::Frame))?;
                self.step(row.each, value, FrameState::Suspended(k))?
            }
            (TerminatorKind::Return, Mode::Resume) => {
                let result = Operand::Place(Place::of(self.body.ret));
                self.step(row.done, &result, FrameState::Done)?
            }
            (TerminatorKind::Ended, Mode::End) => Terminator::Return(None),
            (TerminatorKind::Yield { .. } | TerminatorKind::Return | TerminatorKind::Ended, _) => {
                Terminator::Unreachable
            }
            _ => return Ok(None),
        }))
    }

    /// Leaves `resume`: the step `variant` of `value` given, the frame's state set to `state`.
    fn step(&mut self, variant: Item, value: &Operand, state: FrameState) -> Result<Terminator, Unlowered> {
        let c = self.coroutine.as_ref().ok_or(Unlowered::unsupported(self.at, Form::Frame))?;
        let (step, shape) = (c.step, Arc::clone(&c.shape));
        let temp = self.temp(step)?;
        let kind = Aggregate::Variant { ty: step, name: variant.symbol() };
        self.aggregate(&kind, std::slice::from_ref(value), temp, step)?;
        let frame = self.frame.ok_or(Unlowered::unsupported(self.at, Form::Frame))?;
        let state = self.b.constant(shape.state, shape.stored(state), self.at);
        self.b.store(state, frame, Access::Plain, self.at);
        Ok(match (self.l.types.repr(step, self.at)?, temp) {
            (Repr::Scalar(s), Some(temp)) => {
                Terminator::Return(Some(self.b.load(s, temp, Access::Plain, self.at)))
            }
            (Repr::Memory(layout), Some(temp)) => {
                let out = self.out.ok_or(Unlowered::unsupported(self.at, Form::Frame))?;
                self.b.copy(out, temp, layout.size, self.at);
                Terminator::Return(None)
            }
            _ => Terminator::Return(None),
        })
    }

    /// A frame of the coroutine `body` of instantiation `f` built in `dest`, of type `ty`: not
    /// started, its run-time parameters `args` in their places.
    pub(super) fn construct(
        &mut self,
        f: &Instantiation,
        body: &Body,
        args: &[Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let shape = self.l.frame_shape(ty)?;
        let state = self.b.constant(shape.state, shape.stored(FrameState::Start), self.at);
        self.b.store(state, dest, Access::Plain, self.at);
        let comptime = f.f.comptime(self.l.types.cx).unwrap_or_default();
        let runtime =
            body.params.iter().zip(args).enumerate().filter(|(i, _)| comptime.get(*i) != Some(&true));
        for (_, (&param, arg)) in runtime {
            let Some(offset) = shape.stored[param] else { continue };
            let ty = Self::typed(body, param, self.at)?;
            let at = self.at_offset(dest, offset);
            self.write(arg, Some(at), ty)?;
        }
        Ok(())
    }

    /// `resume` of the frame at `frame`, its step written into `dest`, or `end` of it.
    pub(super) fn drive(&mut self, frame: &Place, dest: Option<Value>, mode: Mode) -> Result<(), Unlowered> {
        let unsupported = Unlowered::unsupported(self.at, Form::Frame);
        let (Some(addr), frame_ty) = self.place(frame)? else { return Err(unsupported) };
        let TyKind::Frame(shaped) = &*frame_ty else { return Err(unsupported) };
        let (resume, end) = self.l.coroutine(&shaped.instantiation());
        let step = self.l.env.resumed(frame_ty).ok_or(unsupported)?;
        let (callee, signature) = match mode {
            Mode::Resume => (resume, self.l.frame_signature(Some(step), self.at)?),
            Mode::End => (end, self.l.frame_signature(None, self.at)?),
        };
        let mut args = vec![addr];
        if signature.through() {
            args.extend(dest);
        }
        let value =
            self.b.push(InstKind::Call { callee: Callee::Func(callee), args }, signature.returns(), self.at);
        if let (Some(value), Some(dest)) = (value, dest) {
            self.b.store(value, dest, Access::Plain, self.at);
        }
        Ok(())
    }
}
