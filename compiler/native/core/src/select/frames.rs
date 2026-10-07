//! Calls and parameters on load and store architectures, which pass values as `vun_abi` places
//! them: a call writes what goes through memory first (arguments on the stack, records copied
//! there, the copies of records passed by address), then each argument passed in a register, calls,
//! and takes its result back; the prologue places the parameters passed in memory, and the entry
//! receives the others; a return passes its value where the convention returns it. What differs
//! between architectures, the instructions, are the hooks of [`Frames`].

use vun_abi::{Carrier, Loc, Part, Passing, Ret};
use vun_backend::{Lack, Missing};
use vun_codegen::snippet::Emits;
use vun_codegen::{Cx, Place};
use vun_lir::{AbiType, CallKind, Callee, Ext, Scalar, Signature, Value};

use super::{Carried, Carries};
use crate::width::offset_of;

/// What a load and store architecture's calls and parameters are written with.
pub trait Frames: Carries {
    /// Where the caller's stack arguments start from the frame pointer, above what the prologue
    /// pushed.
    const CALLER_ARGS: i32;
    /// The stack pointer, which outgoing arguments are stored from.
    const STACK: Self::Reg;

    /// The size an integer of `scalar` is held at.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn sized(cx: &Cx<'_, Self>, scalar: Scalar) -> Result<<Self as Emits>::Size, Lack>;

    /// Puts `v` in `carrier`.
    fn pass(cx: &mut Cx<'_, Self>, v: Value, carrier: Carried<Self>);

    /// Takes the float `v` from the floating-point register `x`.
    fn receive_float(cx: &mut Cx<'_, Self>, v: Value, x: Self::Float);

    /// Extends an integer of `size` in `r` to a word, as the C convention expects.
    fn extend(cx: &mut Cx<'_, Self>, r: Self::Reg, size: <Self as Emits>::Size, ext: Ext);

    /// Writes `a`, of `bytes`, at `offset` of the outgoing arguments.
    fn outgoing(cx: &mut Cx<'_, Self>, a: Value, offset: u64, bytes: u64);

    /// Calls `callee`, through the carrier for one called through an address.
    fn branch_link(cx: &mut Cx<'_, Self>, callee: &Callee);

    /// Leaves the frame and jumps to `callee`, which returns to this function's caller.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn branch_tail(cx: &mut Cx<'_, Self>, callee: &Callee) -> Result<(), Lack>;

    /// Where the entry gathers a result returned in registers, once the prologue reserves it.
    fn gathered(cx: &Cx<'_, Self>) -> Option<i32>;
    fn gather(cx: &mut Cx<'_, Self>, at: i32);

    /// A call of `callee` passing `args`, defining `result`.
    ///
    /// # Errors
    /// What the backend lacks for the call.
    fn call(
        cx: &mut Cx<'_, Self>,
        callee: &Callee,
        args: &[Value],
        kind: CallKind,
        result: Option<Value>,
    ) -> Result<(), Lack> {
        let f = cx.f;
        let signature: &Signature = cx.signature(callee);
        let passing = Passing::of(signature, Self::convention(cx)).map_err(|u| cx.lack(u.into()))?;
        if kind == CallKind::Tail && !passing.replaces(Self::passing(cx)) {
            return Err(cx.lack(Missing::TailCall));
        }
        cx.frame.outgoing(passing.stack);
        // What goes through memory first, through the framework's registers: arguments on the
        // stack, records copied there, and the copies of records passed by address.
        let mut copies = Vec::new();
        for (&a, loc) in args.iter().zip(&passing.params) {
            match loc {
                Loc::Stack { offset, bytes } => Self::outgoing(cx, a, *offset, *bytes),
                Loc::Copied { offset, size } => Self::copy_out(cx, a, 0, *offset, *size),
                Loc::Split { rest: Some(rest), .. } => {
                    Self::copy_out(cx, a, rest.from, rest.offset, rest.bytes);
                }
                Loc::Indirect { size, at } => {
                    let copy = cx.frame.reserve(*size, Self::STACK_ALIGN);
                    cx.reg_to(a, Self::CARRIER);
                    Self::offset(cx, Self::TEMP, Self::FRAME, i64::from(copy));
                    Self::copy_bytes(cx, Self::TEMP, Self::CARRIER, *size, Self::FAR);
                    if let Loc::Stack { offset, bytes } = **at {
                        Self::store_outgoing(cx, Self::TEMP, offset, bytes);
                    }
                    copies.push(copy);
                }
                Loc::Reg(_) | Loc::Split { rest: None, .. } | Loc::Returned => {}
            }
        }
        let mut copies = copies.into_iter();
        // What each LIR value passes, a wide integer's halves both, the address of a result none.
        let abis: Vec<&AbiType> = signature.passed().collect();
        for (i, (&a, loc)) in args.iter().zip(&passing.params).enumerate() {
            match loc {
                Loc::Reg(carrier @ Carrier::Float(_)) => Self::pass(cx, a, *carrier),
                Loc::Reg(Carrier::Int(r)) => match (abis.get(i).copied(), f.constant(a)) {
                    (Some(AbiType::Int(scalar, ext)), Some((_, bits))) => {
                        cx.constant_to(*r, ext.widen(*scalar, bits));
                    }
                    (_, Some((_, bits))) => cx.constant_to(*r, bits),
                    (Some(AbiType::Int(scalar, ext)), None) => {
                        let size = Self::sized(cx, *scalar)?;
                        Self::extend(cx, *r, size, *ext);
                    }
                    // An address known while compiling is written there by the call itself; a
                    // value kept is there already, the plan having put it.
                    (_, None) => {
                        if cx.recomputed(a).is_some() {
                            cx.reg_to(a, *r);
                        }
                    }
                },
                Loc::Split { parts, .. } => {
                    cx.reg_to(a, Self::CARRIER);
                    for part in parts {
                        Self::load_part(
                            cx,
                            part.reg,
                            Self::CARRIER,
                            i64::from(offset_of(part.offset)),
                            part.bytes,
                        );
                    }
                }
                Loc::Indirect { at, .. } => {
                    let copy = copies.next().expect("copied above");
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        Self::offset(cx, r, Self::FRAME, i64::from(copy));
                    }
                }
                Loc::Stack { .. } | Loc::Copied { .. } | Loc::Returned => {}
            }
        }
        if kind == CallKind::Tail {
            // The arguments on the stack go over the ones this function received, after every
            // value is read, since one may come from there. The result is the callee's, which
            // the `return` after it passes on from where it is.
            if passing.stack > 0 {
                Self::offset(cx, Self::TEMP, Self::FRAME, i64::from(Self::CALLER_ARGS));
                Self::offset(cx, Self::CARRIER, Self::STACK, 0);
                Self::copy_bytes(cx, Self::TEMP, Self::CARRIER, passing.stack, Self::FAR);
            }
            return Self::branch_tail(cx, callee);
        }
        Self::branch_link(cx, callee);
        match &passing.ret {
            Ret::Reg(Carrier::Float(x)) => {
                if let Some(out) = result {
                    Self::receive_float(cx, out, *x);
                }
            }
            Ret::Split(parts) => {
                let out = *args.last().expect("the address of the result is passed last");
                cx.reg_to(out, Self::CARRIER);
                for part in parts {
                    Self::store_part(
                        cx,
                        Self::CARRIER,
                        i64::from(offset_of(part.offset)),
                        part.reg,
                        part.bytes,
                    );
                }
            }
            // The plan takes a result in a general register from there.
            Ret::Reg(Carrier::Int(_)) | Ret::None | Ret::Memory => {}
        }
        Ok(())
    }

    /// Copies the `bytes` from `from` of the record at `a` to `offset` of the outgoing arguments.
    fn copy_out(cx: &mut Cx<'_, Self>, a: Value, from: u64, offset: u64, bytes: u64) {
        cx.reg_to(a, Self::CARRIER);
        if from > 0 {
            Self::offset(cx, Self::CARRIER, Self::CARRIER, i64::from(offset_of(from)));
        }
        Self::offset(cx, Self::TEMP, Self::STACK, i64::from(offset_of(offset)));
        Self::copy_bytes(cx, Self::TEMP, Self::CARRIER, bytes, Self::FAR);
    }

    /// Stores the `bytes` low bytes of `r` where the stack argument at `offset` goes.
    fn store_outgoing(cx: &mut Cx<'_, Self>, r: Self::Reg, offset: u64, bytes: u64) {
        Self::store_part(cx, Self::STACK, i64::from(offset_of(offset)), Carrier::Int(r), bytes);
    }

    /// Where each parameter passed in memory is, as the prologue places it: a record the caller
    /// copied, above the frame; one whose parts come in registers, in a buffer; the result
    /// returned in registers, gathered in one.
    fn place_params(cx: &mut Cx<'_, Self>) {
        let f = cx.f;
        let Some(entry) = f.entry() else { return };
        for (i, &v) in f.blocks[entry].params.iter().enumerate() {
            let place = match Self::passing(cx).params[i].clone() {
                Loc::Copied { offset, .. } => Place::frame(Self::CALLER_ARGS + offset_of(offset)),
                Loc::Split { parts, rest } => {
                    Place::frame(Self::buffer(cx, Part::extent(&parts, rest.as_ref())))
                }
                Loc::Returned => {
                    let Ret::Split(parts) = Self::passing(cx).ret.clone() else { continue };
                    let buffer = Self::buffer(cx, Part::extent(&parts, None));
                    Self::gather(cx, buffer);
                    Place::frame(buffer)
                }
                Loc::Reg(_) | Loc::Stack { .. } | Loc::Indirect { .. } => continue,
            };
            cx.recompute_as(v, place);
        }
    }

    /// Receives the parameters passed in floating-point registers and on the stack, and the
    /// records whose parts are passed in registers; those in general registers are where the
    /// plan takes them.
    fn receive_params(cx: &mut Cx<'_, Self>) {
        let f = cx.f;
        let Some(entry) = f.entry() else { return };
        let locs = Self::passing(cx).params.clone();
        for (&v, loc) in f.blocks[entry].params.iter().zip(&locs) {
            // A record copied by the caller is received as its address.
            let loc = match loc {
                Loc::Indirect { at, .. } => at.as_ref(),
                loc => loc,
            };
            match loc {
                Loc::Reg(Carrier::Float(x)) => Self::receive_float(cx, v, *x),
                Loc::Stack { offset, .. } => cx.home_at(v, Self::CALLER_ARGS + offset_of(*offset)),
                Loc::Split { parts, rest } => {
                    let buffer = cx.place(v).expect("placed by the prologue");
                    let at = |offset: u64| buffer.disp + i64::try_from(offset).expect("records fit memory");
                    for part in parts {
                        Self::store_part(cx, Self::FRAME, at(part.offset), part.reg, part.bytes);
                    }
                    // The rest the caller left on the stack joins the parts.
                    if let Some(rest) = rest {
                        Self::offset(cx, Self::TEMP, Self::FRAME, at(rest.from));
                        let above = i64::from(Self::CALLER_ARGS + offset_of(rest.offset));
                        Self::offset(cx, Self::CARRIER, Self::FRAME, above);
                        Self::copy_bytes(cx, Self::TEMP, Self::CARRIER, rest.bytes, Self::FAR);
                    }
                }
                Loc::Reg(Carrier::Int(_)) | Loc::Copied { .. } | Loc::Returned | Loc::Indirect { .. } => {}
            }
        }
    }

    /// Passes `value` where the convention returns it, before the epilogue.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn returned(cx: &mut Cx<'_, Self>, value: Option<Value>) -> Result<(), Lack> {
        let f = cx.f;
        match (value, Self::passing(cx).ret.clone()) {
            (Some(v), Ret::Reg(carrier)) => {
                // A value kept in a general register is there already, the plan having put it.
                if matches!(carrier, Carrier::Float(_)) || cx.recomputed(v).is_some() {
                    Self::pass(cx, v, carrier);
                }
                if let (Some(AbiType::Int(scalar, ext)), Carrier::Int(r)) = (&f.signature.result, carrier) {
                    let size = Self::sized(cx, *scalar)?;
                    Self::extend(cx, r, size, *ext);
                }
            }
            (_, Ret::Split(parts)) => {
                let at = Self::gathered(cx).expect("gathered where the entry receives the address");
                for part in parts {
                    let disp = i64::from(at) + i64::try_from(part.offset).expect("records fit memory");
                    Self::load_part(cx, part.reg, Self::FRAME, disp, part.bytes);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// A buffer in the frame for a record of `size` bytes.
    fn buffer(cx: &mut Cx<'_, Self>, size: u64) -> i32 {
        cx.frame.reserve(size.next_multiple_of(Self::WORD), Self::WORD)
    }
}
