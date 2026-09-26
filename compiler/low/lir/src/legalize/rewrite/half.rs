//! Operations on a promoted `f16`: arithmetic and comparisons in `f32` between the exact widening
//! and the rounding back, conversions through `f32`, the encoding its bits as they are.

use vun_origin::OriginId;

use super::super::Unlegalized;
use super::super::float16::Float16;
use super::{Parts, Rewrite};
use crate::{CastOp, InstKind, Scalar, Value};

impl Rewrite<'_> {
    /// `kind` rewritten when it reads or gives a promoted `f16`; `None` for another.
    pub(super) fn half(&mut self, kind: &InstKind, at: OriginId) -> Result<Option<Parts>, Unlegalized> {
        let promoted = |v: &Value| self.legalize.promoted(self.old.ty(*v));
        let value = match kind {
            InstKind::Binary { op, a, b } if promoted(a) => {
                let (x, y) = (self.widened(*a, at), self.widened(*b, at));
                let exact = self.b.binary(*op, x, y, at);
                Float16 { b: &mut self.b, at }.narrow(exact)
            }
            InstKind::Compare { cond, a, b } if promoted(a) => {
                let (x, y) = (self.widened(*a, at), self.widened(*b, at));
                self.b.compare(*cond, x, y, at)
            }
            InstKind::Cast { op, value, to } if promoted(value) || self.legalize.promoted(*to) => {
                let from = self.old.ty(*value);
                match op {
                    CastOp::Bitcast => self.one(*value),
                    CastOp::FConvert | CastOp::FToS | CastOp::FToU if promoted(value) => {
                        let wide = self.widened(*value, at);
                        if *to == Scalar::F32 { wide } else { self.b.cast(*op, wide, *to, at) }
                    }
                    // Rounding once: only from `f32`, since rounding through it would round twice.
                    CastOp::FConvert if from == Scalar::F32 => {
                        let x = self.one(*value);
                        Float16 { b: &mut self.b, at }.narrow(x)
                    }
                    _ => return Err(Unlegalized::HalfConversion),
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Parts::One(value)))
    }

    /// The `f32` equal to the promoted `f16` `v`.
    fn widened(&mut self, v: Value, at: OriginId) -> Value {
        let bits = self.one(v);
        Float16 { b: &mut self.b, at }.widen(bits)
    }
}
