//! The parameters of a function on load and store architectures, as `vun_abi` places them: the
//! prologue places the ones passed in memory, the entry receives the others, and a return passes
//! its value where the convention returns it.

use vun_abi::{Carrier, Loc, Part, Ret};
use vun_backend::Lack;
use vun_codegen::{Cx, Place};
use vun_lir::{AbiType, Value};

use super::Frames;
use crate::width::offset_of;

pub trait Params: Frames {
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

impl<A: Frames> Params for A {}
