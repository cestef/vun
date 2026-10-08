//! Calls and parameters on load and store architectures, which pass values as `vun_abi` places
//! them: a call writes what goes through memory first (arguments on the stack, records copied
//! there, the copies of records passed by address), then each argument passed in a register, calls,
//! and takes its result back; the prologue places the parameters passed in memory, and the entry
//! receives the others; a return passes its value where the convention returns it. What differs
//! between architectures, the instructions, are the hooks of [`Frames`].

use vun_abi::{Carrier, Loc, Passing, Ret};
use vun_backend::{Lack, Missing};
use vun_codegen::Cx;
use vun_codegen::snippet::Emits;
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
        if kind == CallKind::Tail {
            let received = f.entry().map(|entry| f.blocks[entry].params.as_slice()).unwrap_or_default();
            if !passing.reuses(Self::passing(cx), args, received) {
                return Err(cx.lack(Missing::TailCall));
            }
        }
        cx.frame.outgoing(passing.stack).map_err(|m| cx.lack(m))?;
        let copies = Self::write_memory(cx, args, &passing, kind);
        Self::load_registers(cx, signature, args, &passing, copies, kind)?;
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

    /// Writes what goes through memory first, through the framework's registers: arguments on the
    /// stack, records copied there, and the copies of records passed by address, whose offsets in
    /// the frame it returns.
    fn write_memory(
        cx: &mut Cx<'_, Self>,
        args: &[Value],
        passing: &Passing<Carried<Self>>,
        kind: CallKind,
    ) -> Vec<i32> {
        let mut copies = Vec::new();
        for (&a, loc) in args.iter().zip(&passing.params) {
            match loc {
                Loc::Stack { offset, bytes } => Self::outgoing(cx, a, *offset, *bytes),
                Loc::Copied { offset, size } => Self::copy_out(cx, a, 0, *offset, *size),
                Loc::Split { rest: Some(rest), .. } => {
                    Self::copy_out(cx, a, rest.from, rest.offset, rest.bytes);
                }
                // A record the function received, passed on as it is.
                Loc::Indirect { at, .. } if kind == CallKind::Tail => {
                    if let Loc::Stack { offset, bytes } = **at {
                        cx.reg_to(a, Self::CARRIER);
                        Self::store_outgoing(cx, Self::CARRIER, offset, bytes);
                    }
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
        copies
    }

    /// Puts each argument passed in a register there, and the copies of records passed by address
    /// in `copies`.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn load_registers(
        cx: &mut Cx<'_, Self>,
        signature: &Signature,
        args: &[Value],
        passing: &Passing<Carried<Self>>,
        copies: Vec<i32>,
        kind: CallKind,
    ) -> Result<(), Lack> {
        let f = cx.f;
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
                Loc::Indirect { at, .. } if kind == CallKind::Tail => {
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        cx.reg_to(a, r);
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
}
