//! Borrow types (appendix B, the table `borrows`): `&T` and the other modes, each a `lang.mem`
//! constructor, with the transition, access and extent its row gives.

use std::collections::HashMap;
use std::sync::LazyLock;

use vun_desugar::{Borrow, Extent};

use super::{Ty, TyKind, Value};
use crate::Primitive;
use crate::int::{Integer, Interval};

impl TyKind {
    /// The type a borrow type borrows: `T` for `&T`, `&mut T` and the other modes of the table
    /// `borrows` (appendix B), which have no namespace of their own (spec 01, section 4).
    #[must_use]
    pub fn borrowed(&self) -> Option<Ty> {
        self.borrow().map(|(_, ty)| ty)
    }

    /// The borrow type constructor and the type borrowed, for a borrow type.
    #[must_use]
    pub fn borrow(&self) -> Option<(Primitive, Ty)> {
        self.borrowing().map(|(_, p, ty)| (p, ty))
    }

    /// For a borrow type: its mode, its constructor and the type borrowed.
    #[must_use]
    pub fn borrowing(&self) -> Option<(Borrow, Primitive, Ty)> {
        let Self::Primitive(p, args) = self else { return None };
        let [Value::Type(ty)] = args.as_slice() else { return None };
        Some((p.borrow_mode()?, *p, *ty))
    }

    /// `&mode pointee`.
    #[must_use]
    pub fn borrowed_as(mode: Borrow, pointee: Ty) -> Option<Ty> {
        Some(Self::Primitive(Primitive::at(mode.item())?, vec![Value::Type(pointee)]).intern())
    }

    /// The type reached through every borrow this type is, as `.` reaches it (spec 01,
    /// section 4): `u8` for `&&u8`.
    #[must_use]
    pub fn through_borrows(&self) -> Ty {
        let mut ty = self.clone().intern();
        while let Some(pointee) = ty.borrowed() {
            ty = pointee;
        }
        ty
    }

    /// How long a call holds a borrow of this type (the table `borrows`).
    #[must_use]
    pub fn extent(&self) -> Option<Extent> {
        self.borrowing().map(|(mode, ..)| mode.extent())
    }

    /// `&static [len]u8`: the type of a string literal of `len` bytes (spec 01, section 7).
    #[must_use]
    pub fn string(len: usize) -> Option<Ty> {
        let byte = Self::int(&Interval::closed(Integer::from(0), Integer::from(i128::from(u8::MAX)))?);
        let bytes = Self::array(Integer::from(i128::try_from(len).ok()?), byte);
        Self::borrowed_as(Borrow::plain().lasting()?, bytes)
    }
}

impl Primitive {
    /// The primitive `item` is, as tables name them: `lang.mem.Ref`.
    #[must_use]
    pub fn at(item: vun_items::Item) -> Option<Self> {
        static BY_ITEM: LazyLock<HashMap<vun_items::Item, Primitive>> =
            LazyLock::new(|| Primitive::all().map(|p| (p.item(), p)).collect());
        BY_ITEM.get(&item).copied()
    }

    /// The borrow mode whose type constructor this is (the table `borrows`).
    #[must_use]
    pub fn borrow_mode(self) -> Option<Borrow> {
        Borrow::of(self.item())
    }
}
