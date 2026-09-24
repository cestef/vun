//! Function types: `(A, B) -> R` is `lang.Fn((A, B), R)` (appendix B). A function type also
//! carries its markers (spec 07, section 4), as a third argument when there are some; calls are
//! checked against them.

use overt_grammar::TokenKind;

use super::marker::Marker;
use super::{Ty, TyKind, Value};
use crate::Primitive;
use crate::primitives::lang::Lang;
use crate::primitives::safety::Safety;

/// The parts of a function type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature {
    pub params: Vec<Ty>,
    pub result: Ty,
    /// The markers on the type, in the order of the table `fn_markers`.
    pub markers: Vec<Marker>,
}

impl Signature {
    /// An unmarked function's signature.
    #[must_use]
    pub fn new(params: Vec<Ty>, result: Ty) -> Self {
        Self { params, result, markers: Vec::new() }
    }

    /// `lang.Fn((params), result)`, with its markers after when there are some.
    #[must_use]
    pub fn ty(&self) -> Ty {
        let mut args =
            vec![Value::Type(TyKind::Tuple(self.params.clone()).intern()), Value::Type(self.result)];
        if !self.markers.is_empty() {
            args.push(Value::Tuple(self.markers.iter().map(Marker::value).collect()));
        }
        TyKind::Primitive(Primitive::Lang(Lang::Fn), args).intern()
    }

    /// This signature with the marker the prefix `token` puts, if it puts one.
    #[must_use]
    pub fn marked(self, token: TokenKind) -> Self {
        match Marker::of_prefix(token) {
            Some(marker) => self.with(marker),
            None => self,
        }
    }

    /// This signature with `marker` too.
    #[must_use]
    pub fn with(mut self, marker: Marker) -> Self {
        Marker::insert(&mut self.markers, marker);
        self
    }

    /// Whether this signature carries the marker `item`.
    #[must_use]
    pub fn has(&self, item: Primitive) -> bool {
        self.markers.iter().any(|m| m.item == item)
    }

    /// Whether calling a function of this signature is `unsafe`.
    #[must_use]
    pub fn is_unsafe(&self) -> bool {
        self.has(Primitive::Safety(Safety::r#unsafe))
    }

    /// Whether a function of this signature promises never to trap.
    #[must_use]
    pub fn never_traps(&self) -> bool {
        Marker::never_traps(&self.markers)
    }

    /// Whether a function of signature `found` may be used where one of this signature is: the
    /// same parameters and result, and markers related as the table `fn_markers` says.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        self.params == found.params
            && self.result == found.result
            && Marker::accepts(&self.markers, &found.markers)
    }
}

impl TyKind {
    /// The parts of a function type.
    #[must_use]
    pub fn signature(&self) -> Option<Signature> {
        let Self::Primitive(Primitive::Lang(Lang::Fn), args) = self else { return None };
        let (params, result, markers) = match args.as_slice() {
            [Value::Type(params), Value::Type(result)] => (params, result, Vec::new()),
            [Value::Type(params), Value::Type(result), Value::Tuple(markers)] => {
                (params, result, markers.iter().map(Marker::from_value).collect::<Option<_>>()?)
            }
            _ => return None,
        };
        let Self::Tuple(params) = &**params else { return None };
        Some(Signature { params: params.clone(), result: *result, markers })
    }

    /// The signature a value of this type is called with: a function's, or a closure's function's
    /// without its captures.
    #[must_use]
    pub fn callable(&self) -> Option<Signature> {
        match self {
            Self::Closure(closure) => Some(closure.signature.clone()),
            _ => self.signature(),
        }
    }

    /// `()`, the empty tuple: the result of a function written without one.
    #[must_use]
    pub fn unit() -> Ty {
        Self::Tuple(Vec::new()).intern()
    }
}
