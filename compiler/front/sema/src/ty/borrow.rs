//! Borrow types (appendix B, the table `borrows`): `&T` and the other modes, each a `lang.mem`
//! constructor named by the words after `&`, with the transition, access and extent its row gives.

use vun_desugar::{BORROWS, Extent};

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

    /// For a borrow type: the word after `&` that writes it (the table `borrows`), its
    /// constructor and the type borrowed.
    #[must_use]
    pub fn borrowing(&self) -> Option<(&'static str, Primitive, Ty)> {
        let Self::Primitive(p, args) = self else { return None };
        let [Value::Type(ty)] = args.as_slice() else { return None };
        Some((p.borrow_mode()?, *p, *ty))
    }

    /// `&mode pointee`: the borrow type written with the words `mode` after `&`.
    #[must_use]
    pub fn borrowed_as(mode: &str, pointee: Ty) -> Option<Ty> {
        let &(_, item, ..) = BORROWS.iter().find(|(m, ..)| *m == mode)?;
        Some(Self::Primitive(Primitive::at(item)?, vec![Value::Type(pointee)]).intern())
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

    /// Whether the borrow modes `a` and `b` perform one transition and leave one access (the
    /// table `borrows`), differing at most in how long a call holds them.
    #[must_use]
    pub fn alike(a: &str, b: &str) -> bool {
        let row = |mode: &str| {
            BORROWS
                .iter()
                .find(|(m, ..)| *m == mode)
                .map(|&(_, _, before, after, access, _)| (before, after, access))
        };
        row(a).is_some() && row(a) == row(b)
    }

    /// How long a call holds a borrow of this type (the table `borrows`).
    #[must_use]
    pub fn extent(&self) -> Option<Extent> {
        let (mode, ..) = self.borrowing()?;
        BORROWS.iter().find(|(m, ..)| *m == mode).map(|&(.., extent)| extent)
    }

    /// `&static [len]u8`: the type of a string literal of `len` bytes (spec 01, section 7).
    #[must_use]
    pub fn string(len: usize) -> Option<Ty> {
        let (_, item, ..) = BORROWS.iter().find(|&&(.., extent)| extent == Extent::Static)?;
        let byte = Self::int(&Interval::closed(Integer::from(0), Integer::from(i128::from(u8::MAX)))?);
        let bytes = Self::array(Integer::from(i128::try_from(len).ok()?), byte);
        Some(Self::Primitive(Primitive::at(*item)?, vec![Value::Type(bytes)]).intern())
    }
}

impl Primitive {
    /// The primitive `item` is, as tables name them: `lang.mem.Ref`.
    #[must_use]
    pub fn at(item: vun_items::Item) -> Option<Self> {
        Self::all().find(|p| p.item() == item)
    }

    /// The words after `&` that write this borrow type constructor (the table `borrows`).
    #[must_use]
    pub fn borrow_mode(self) -> Option<&'static str> {
        BORROWS.iter().find_map(|&(mode, item, ..)| (Self::at(item) == Some(self)).then_some(mode))
    }
}
