//! Function types: `(A, B) -> R` is `lang.Fn((A, B), R)` (appendix B).

use super::{Ty, TyKind, Value};
use crate::Primitive;
use crate::primitives::lang::Lang;

/// The parts of a function type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature {
    pub params: Vec<Ty>,
    pub result: Ty,
}

impl Signature {
    /// `lang.Fn((params), result)`.
    #[must_use]
    pub fn ty(&self) -> Ty {
        let params = Value::Type(TyKind::Tuple(self.params.clone()).intern());
        TyKind::Primitive(Primitive::Lang(Lang::Fn), vec![params, Value::Type(self.result)]).intern()
    }
}

impl TyKind {
    /// The parts of a function type.
    #[must_use]
    pub fn signature(&self) -> Option<Signature> {
        let Self::Primitive(Primitive::Lang(Lang::Fn), args) = self else { return None };
        let [Value::Type(params), Value::Type(result)] = args.as_slice() else { return None };
        let Self::Tuple(params) = &**params else { return None };
        Some(Signature { params: params.clone(), result: *result })
    }

    /// `()`, the empty tuple: the result of a function written without one.
    #[must_use]
    pub fn unit() -> Ty {
        Self::Tuple(Vec::new()).intern()
    }
}
