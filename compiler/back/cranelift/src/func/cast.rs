//! Conversions between scalars.

use cranelift_codegen::ir::{InstBuilder, Value as Val, types};
use vun_backend::Lack;
use vun_lir::{CastOp, Scalar};
use vun_origin::OriginId;

use super::Lower;

impl Lower<'_, '_> {
    pub(super) fn cast(&mut self, op: CastOp, v: Val, to: Scalar, at: OriginId) -> Result<Val, Lack> {
        let to = self.ty(to, at)?;
        let from_ty = self.b.func.dfg.value_type(v);
        let same = from_ty == to;
        Ok(match op {
            CastOp::ZExt => self.b.ins().uextend(to, v),
            CastOp::SExt => self.b.ins().sextend(to, v),
            CastOp::Trunc => self.b.ins().ireduce(to, v),
            CastOp::FConvert if from_ty.bits() < to.bits() => self.b.ins().fpromote(to, v),
            CastOp::FConvert => self.b.ins().fdemote(to, v),
            CastOp::SToF => {
                let wide = self.widened(v, true);
                self.b.ins().fcvt_from_sint(to, wide)
            }
            CastOp::UToF => {
                let wide = self.widened(v, false);
                self.b.ins().fcvt_from_uint(to, wide)
            }
            CastOp::FToS => self.saturating(to, v, true),
            CastOp::FToU => self.saturating(to, v, false),
            CastOp::PtrToInt | CastOp::IntToPtr => v,
            CastOp::Bitcast if same => v,
            CastOp::Bitcast => {
                let flags = Self::plain();
                self.b.ins().bitcast(to, flags, v)
            }
        })
    }

    /// `v` as an integer of at least 32 bits, the narrowest Cranelift converts to floats.
    fn widened(&mut self, v: Val, signed: bool) -> Val {
        let ty = self.b.func.dfg.value_type(v);
        match (ty.bits() < 32, signed) {
            (false, _) => v,
            (true, true) => self.b.ins().sextend(types::I32, v),
            (true, false) => self.b.ins().uextend(types::I32, v),
        }
    }

    /// The float `v` as an integer `to`, in the range it fits, the narrowest results taken from
    /// 32 bits.
    fn saturating(&mut self, to: cranelift_codegen::ir::Type, v: Val, signed: bool) -> Val {
        let wide = if to.bits() < 32 { types::I32 } else { to };
        let converted = if signed {
            self.b.ins().fcvt_to_sint_sat(wide, v)
        } else {
            self.b.ins().fcvt_to_uint_sat(wide, v)
        };
        if wide == to { converted } else { self.b.ins().ireduce(to, converted) }
    }
}
