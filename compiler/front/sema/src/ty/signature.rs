//! Function types: `(A, B) -> R` is `lang.Fn((A, B), R)` (appendix B). A function type also
//! carries the markers its prefixes put on it (the table `markers`), such as the
//! `lang.safety.unsafe` of `unsafe (A) -> R`, as a third argument when there are some; calls are
//! checked against them (spec 07, section 4).

use overt_grammar::{MARKERS, PATH_SEPARATOR, TokenKind};

use super::{Ty, TyKind, Value};
use crate::Primitive;
use crate::primitives::lang::Lang;

/// The parts of a function type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature {
    pub params: Vec<Ty>,
    pub result: Ty,
    /// The markers on the type, in the order of the table `markers`.
    pub markers: Vec<Primitive>,
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
            args.push(Value::Tuple(self.markers.iter().map(|&m| Value::Primitive(m)).collect()));
        }
        TyKind::Primitive(Primitive::Lang(Lang::Fn), args).intern()
    }

    /// This signature with the marker the prefix `token` puts, if it puts one.
    #[must_use]
    pub fn marked(mut self, token: TokenKind) -> Self {
        if let Some(marker) = Self::marker(token)
            && !self.markers.contains(&marker)
        {
            self.markers.push(marker);
            let order = |m: &Primitive| MARKERS.iter().position(|(t, _)| Self::marker(*t) == Some(*m));
            self.markers.sort_by_key(order);
        }
        self
    }

    /// Whether calling a function of this signature is `unsafe`.
    #[must_use]
    pub fn is_unsafe(&self) -> bool {
        Self::marker(TokenKind::Unsafe).is_some_and(|m| self.markers.contains(&m))
    }

    /// The marker the prefix `token` puts on a function type (the table `markers`).
    #[must_use]
    pub fn marker(token: TokenKind) -> Option<Primitive> {
        let (_, item) = MARKERS.iter().find(|(t, _)| *t == token)?;
        let (ns, name) = item.rsplit_once(PATH_SEPARATOR)?;
        Primitive::ALL.iter().copied().find(|p| p.namespace() == ns && p.name() == name)
    }

    /// Whether a function of signature `found` may be used where one of this signature is: the
    /// same parameters and result, and no marker this one lacks, since each marker is a duty
    /// its callers take on.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        self.params == found.params
            && self.result == found.result
            && found.markers.iter().all(|m| self.markers.contains(m))
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
                let markers = markers.iter().map(|m| match m {
                    Value::Primitive(p) => Some(*p),
                    _ => None,
                });
                (params, result, markers.collect::<Option<_>>()?)
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
