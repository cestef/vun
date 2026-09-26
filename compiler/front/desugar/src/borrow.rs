//! Borrow modes (appendix B, the table `borrows`): each mode is a row of the table, and every fact
//! about it is read from its columns.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_items::Item;

use crate::{Access, BORROWS, BorrowsRow, Extent, PlaceState};

/// Separates the words of a borrow mode in the table `borrows`: `temp mut`.
pub const MODE_SEPARATOR: &str = " ";

/// A borrow mode: a row of the table `borrows`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct Borrow(u8);

impl Borrow {
    /// Every mode, in the table's order.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..=u8::MAX).take(BORROWS.len()).map(Self)
    }

    /// The mode written with the words `words` after `&`; `&T` itself for none.
    #[must_use]
    pub fn written<'w>(words: impl IntoIterator<Item = &'w str>) -> Option<Self> {
        let words: Vec<&str> = words.into_iter().collect();
        Self::all()
            .find(|b| b.row().mode.split(MODE_SEPARATOR).filter(|w| !w.is_empty()).eq(words.iter().copied()))
    }

    /// The mode whose type is the `lang` item `item`.
    #[must_use]
    pub fn of(item: Item) -> Option<Self> {
        Self::all().find(|b| b.item() == item)
    }

    /// `&T`, written with no word.
    ///
    /// # Panics
    /// If the table `borrows` has no row without words.
    #[must_use]
    pub fn plain() -> Self {
        Self::written([]).expect("the table `borrows` has `&T`")
    }

    /// The mode of the same transition and access for what lives as long as the program: the
    /// static region (spec 05, section 4).
    #[must_use]
    pub fn lasting(self) -> Option<Self> {
        Self::all().find(|b| b.alike(self) && b.extent() == Extent::Static)
    }

    fn row(self) -> &'static BorrowsRow {
        &BORROWS[usize::from(self.0)]
    }

    /// The `lang` item its borrow type is.
    #[must_use]
    pub fn item(self) -> Item {
        self.row().item
    }

    /// The state the borrowed place needs.
    #[must_use]
    pub fn before(self) -> PlaceState {
        self.row().before
    }

    /// The state the borrowed place has once the borrow ends.
    #[must_use]
    pub fn after(self) -> PlaceState {
        self.row().after
    }

    /// The access it leaves others.
    #[must_use]
    pub fn access(self) -> Access {
        self.row().access
    }

    /// How long a call holds it.
    #[must_use]
    pub fn extent(self) -> Extent {
        self.row().extent
    }

    /// Whether `self` and `other` perform one transition and leave one access, so that each is
    /// accepted for the other.
    #[must_use]
    pub fn alike(self, other: Self) -> bool {
        (self.before(), self.after(), self.access()) == (other.before(), other.after(), other.access())
    }

    /// Whether it is written with no word: `&T`.
    #[must_use]
    pub fn is_plain(self) -> bool {
        self.row().mode.is_empty()
    }
}

/// The words after `&`, as written.
impl fmt::Display for Borrow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.row().mode)
    }
}
