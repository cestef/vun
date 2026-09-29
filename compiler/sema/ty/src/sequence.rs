//! Arrays, slices and vectors: their types built and taken apart.

use vun_primitives::Primitive;
use vun_primitives::mem::Mem;
use vun_primitives::simd::Simd;
use vun_sema_int::Integer;

use crate::{Ty, TyKind, Value};

impl TyKind {
    /// `lang.mem.Array(len, elem)`: `[len]elem`.
    #[must_use]
    pub fn array(len: Integer, elem: Ty) -> Ty {
        Self::Primitive(Primitive::Mem(Mem::Array), vec![Value::Int(len), Value::Type(elem)]).intern()
    }

    /// `lang.mem.Slice(elem)`: `[elem]`.
    #[must_use]
    pub fn slice(elem: Ty) -> Ty {
        Self::Primitive(Primitive::Mem(Mem::Slice), vec![Value::Type(elem)]).intern()
    }

    /// The length and item type of an array type.
    #[must_use]
    pub fn as_array(&self) -> Option<(&Integer, Ty)> {
        match self {
            Self::Primitive(Primitive::Mem(Mem::Array), args) => match args.as_slice() {
                [Value::Int(len), Value::Type(elem)] => Some((len, *elem)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The number of lanes and the lane type of a vector type, `lang.simd.Vector(n, T)`.
    #[must_use]
    pub fn lanes(&self) -> Option<(&Integer, Ty)> {
        match self {
            Self::Primitive(Primitive::Simd(Simd::Vector), args) => match args.as_slice() {
                [Value::Int(lanes), Value::Type(lane)] => Some((lanes, *lane)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The item type of a slice type.
    #[must_use]
    pub fn slice_items(&self) -> Option<Ty> {
        match self {
            Self::Primitive(Primitive::Mem(Mem::Slice), args) => match args.as_slice() {
                [Value::Type(elem)] => Some(*elem),
                _ => None,
            },
            _ => None,
        }
    }

    /// The item type of an array or slice type.
    #[must_use]
    pub fn items(&self) -> Option<Ty> {
        self.slice_items().or_else(|| self.as_array().map(|(_, elem)| elem))
    }

    /// The count and item type of a type laid out as that many items in a row: an array's, or a
    /// vector's lanes.
    #[must_use]
    pub fn repeated(&self) -> Option<(&Integer, Ty)> {
        self.as_array().or_else(|| self.lanes())
    }
}
