//! Operations on a float held as its bits: a bitcast is the bits as they are; loads, stores,
//! selects and calls move them as any integer's; any other operation ran as `lang.float.soft`'s
//! before lowering, so none is left.

use super::super::Unlegalized;
use super::{Parts, Rewrite};
use vun_lir::{CastOp, InstKind, Value};

impl Rewrite<'_> {
    /// `kind` rewritten when it computes with a float held as its bits; `None` for another.
    pub(super) fn held(&self, kind: &InstKind) -> Result<Option<Parts>, Unlegalized> {
        let held = |v: &Value| self.legalize.held(self.old.ty(*v)).is_some();
        match kind {
            InstKind::Cast { op: CastOp::Bitcast, value, to }
                if held(value) || self.legalize.held(*to).is_some() =>
            {
                Ok(Some(self.parts[value].clone()))
            }
            InstKind::Cast { value, to, .. } if held(value) || self.legalize.held(*to).is_some() => {
                Err(Unlegalized::HeldFloat)
            }
            InstKind::Binary { a, .. } | InstKind::Compare { a, .. } if held(a) => {
                Err(Unlegalized::HeldFloat)
            }
            _ => Ok(None),
        }
    }
}
