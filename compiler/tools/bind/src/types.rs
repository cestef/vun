//! C types as Vun writes them (spec 07, section 5), by what clang says of each on the target: an
//! integer by its size and signedness, a pointer as `?*T` since C may pass null, a function
//! pointer as `?#[abi.c] (...) -> R`, a record, enum or typedef by the name it is bound under.

use vun_clang::{Cursor, Kind, Type, TypeKind};
use vun_syntax::make::{Expr, Field};

use crate::{Binder, Opacity, items};

/// Why a type has no Vun spelling here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// A C type Vun has no counterpart for, as C spells it: `long double`, a union, `...`.
    Type(String),
    /// A struct bound without its fields, used by value where they matter: an incomplete one
    /// anywhere, one bound as bytes in a call, whose convention classifies the fields.
    Opaque(String),
}

/// Where a type is written, which decides what `void`, `T[]` and an opaque struct mean.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Position {
    /// A function's result: `void` is no result.
    Result,
    /// A parameter: `T[]` is a pointer.
    Param,
    /// A field or an array item.
    Field,
}

impl Binder<'_> {
    /// The Vun type for `ty` written at `at`; `None` for a result of `void`.
    pub(crate) fn ty(&self, ty: Type<'_>, at: Position) -> Result<Option<Expr>, Unsupported> {
        let unsupported = || Unsupported::Type(ty.spelling());
        let made = match ty.kind() {
            TypeKind::Void if at == Position::Result => return Ok(None),
            TypeKind::Bool => items::condition(),
            TypeKind::Integer { signed } => {
                let bytes = ty.size().filter(|s| matches!(s, 1 | 2 | 4 | 8)).ok_or_else(unsupported)?;
                items::integer(signed, bytes * u64::from(u8::BITS))
            }
            TypeKind::Float => {
                ty.size().and_then(|s| items::float(s * u64::from(u8::BITS))).ok_or_else(unsupported)?
            }
            TypeKind::Pointer => self.pointer(ty.pointee())?,
            TypeKind::IncompleteArray if at == Position::Param => self.pointer(ty.element())?,
            TypeKind::Array => {
                let item = self.ty(ty.element(), Position::Field)?.ok_or_else(unsupported)?;
                let len = ty.array_len().ok_or_else(unsupported)?;
                Expr::Array(Box::new(Expr::Int(i128::from(len))), Box::new(item))
            }
            TypeKind::Record | TypeKind::Enum => {
                let decl = ty.declaration();
                let name = self.bound(&decl).ok_or_else(unsupported)?;
                match (self.opaque.get(&name), at) {
                    (None | Some(Opacity::Sized), Position::Field) => Expr::local(&name),
                    (None, _) if self.passable(&decl) => Expr::local(&name),
                    _ => return Err(Unsupported::Opaque(name)),
                }
            }
            TypeKind::Typedef => match self.bound(&ty.declaration()) {
                // Checked as what it stands for, written by its name.
                Some(name) => {
                    self.ty(ty.canonical(), at)?;
                    Expr::local(&name)
                }
                // A typedef not bound here, such as `size_t`: what it stands for.
                None => return self.ty(ty.canonical(), at),
            },
            TypeKind::Void | TypeKind::IncompleteArray | TypeKind::Function | TypeKind::Other => {
                return Err(unsupported());
            }
        };
        Ok(Some(made))
    }

    /// `?*T`, `?*()` for a pointer to `void` or to what is not bound here, or
    /// `?#[lang.abi.c] (...) -> R` for a pointer to a function. Behind a pointer, a struct's
    /// fields never matter.
    fn pointer(&self, pointee: Type<'_>) -> Result<Expr, Unsupported> {
        if pointee.canonical().kind() == TypeKind::Function {
            return Ok(items::option(self.function(pointee.canonical())?));
        }
        let target = match pointee.kind() {
            TypeKind::Void => Expr::Unit,
            TypeKind::Record | TypeKind::Enum | TypeKind::Typedef => match self.bound(&pointee.declaration())
            {
                Some(name) => Expr::local(&name),
                None if pointee.kind() == TypeKind::Typedef => return self.pointer(pointee.canonical()),
                None => Expr::Unit,
            },
            _ => self.ty(pointee, Position::Field)?.unwrap_or(Expr::Unit),
        };
        Ok(items::option(items::pointer(target)))
    }

    /// Whether a record can be passed by value: the calling convention classifies its fields,
    /// so none of them, nested, may be bound as bytes.
    fn passable(&self, record: &Cursor<'_>) -> bool {
        record.children().iter().filter(|f| f.kind() == Kind::Field).all(|f| {
            let mut ty = f.ty().canonical();
            while ty.kind() == TypeKind::Array {
                ty = ty.element().canonical();
            }
            if ty.kind() != TypeKind::Record {
                return true;
            }
            let decl = ty.declaration();
            self.bound(&decl).is_none_or(|name| !self.opaque.contains_key(&name)) && self.passable(&decl)
        })
    }

    /// A function type with the C calling convention, `#[lang.abi.c] (...) -> R`; its
    /// parameters are named by position, since a type has no parameter names.
    pub(crate) fn function(&self, ty: Type<'_>) -> Result<Expr, Unsupported> {
        if ty.is_variadic() {
            return Err(Unsupported::Type(ty.spelling()));
        }
        let params = ty
            .params()
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let ty = self.ty(p, Position::Param)?.unwrap_or(Expr::Unit);
                Ok(Field { name: Self::positional(i), ty })
            })
            .collect::<Result<Vec<_>, Unsupported>>()?;
        let result = self.ty(ty.result(), Position::Result)?.map(Box::new);
        Ok(items::c_abi(Expr::Fn(params, result)))
    }

    /// The name of the parameter at `i` that C leaves unnamed.
    pub(crate) fn positional(i: usize) -> String {
        [POSITIONAL, &i.to_string()].concat()
    }
}

/// What unnamed parameters are named by, before their position.
const POSITIONAL: &str = "p";
