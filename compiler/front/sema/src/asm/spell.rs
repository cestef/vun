//! How mnemonics spell forms, as each architecture's assembler writes them: a word, a condition
//! glued to it (`jz`, `beq`) or after a `.` (`b.eq`), a width's suffix glued (`ldrb`), and
//! qualifiers each after a `.` (`add.w`, `vcvt.s32.f32`). Each architecture composes the steps
//! its syntax takes; what they fix is what the spelling says of the form.

use vun_codec::{Decode, Encode};
use vun_inline::{CondId, SizeId};
use vun_intern::Symbol;
use vun_isa::Fixed;
use vun_items::Item;

use super::table::Table;
use crate::target::Members;
use crate::ty::Value;

/// A mnemonic as a line writes it: a word, and what follows each `.` in order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct Mnemonic {
    pub word: Symbol,
    pub parts: Vec<Symbol>,
}

/// Where a part of a mnemonic is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum At {
    /// Glued to the word: `jz`, `beq`.
    Glued,
    /// After a `.`: `b.eq`.
    Dotted,
}

/// A width a form is defined at, as its spellings write it.
pub(super) struct Width {
    size: Option<SizeId>,
    /// Glued to the word: `ldrb`.
    suffix: Option<String>,
    /// Each after a `.`: `vcvt.s32.f32`.
    qualifiers: Vec<Symbol>,
}

/// The fields of an architecture's `Sized` a width is read from.
pub(super) struct Sizes {
    /// The form's field listing its widths.
    pub(super) of: Item,
    pub(super) size: Item,
    pub(super) suffix: Option<Item>,
    pub(super) qualifiers: Option<Item>,
}

impl Width {
    /// The widths `form` is defined at, as `sizes` reads them from `table`'s architecture.
    pub(super) fn of(table: &Table, form: &Value, sizes: &Sizes) -> Vec<Self> {
        let field = |value: &Value, item: Item| Members::of(value).ok()?.get(item).ok().cloned();
        let listed = field(form, sizes.of).and_then(Value::items).unwrap_or_default();
        listed
            .iter()
            .map(|s| {
                let size = field(s, sizes.size).and_then(|v| match v {
                    Value::Variant { name, .. } => table.size(name),
                    _ => None,
                });
                let suffix = sizes.suffix.and_then(|f| field(s, f)?.text()).filter(|t| !t.is_empty());
                let qualifiers = sizes.qualifiers.and_then(|f| field(s, f)?.items()).unwrap_or_default();
                let qualifiers =
                    qualifiers.iter().filter_map(Value::text).map(|q| Symbol::intern(&q)).collect();
                Self { size, suffix, qualifiers }
            })
            .collect()
    }

    /// Whether its spelling names it apart from the form's other widths.
    fn named(&self) -> bool {
        self.suffix.is_some() || !self.qualifiers.is_empty()
    }
}

/// The spellings of a form so far, each with what it fixes.
pub(super) struct Spelled(Vec<(Mnemonic, Fixed)>);

impl Spelled {
    /// The word alone.
    pub(super) fn of(word: Symbol) -> Self {
        Self(vec![(Mnemonic { word, parts: Vec::new() }, Fixed::default())])
    }

    /// Each spelling with each of `table`'s conditions written at `at`, fixing it.
    pub(super) fn conds(self, table: &Table, at: At) -> Self {
        let conds: Vec<(CondId, Symbol)> = table
            .conds
            .iter()
            .enumerate()
            .filter_map(|(i, &c)| Some((CondId(u8::try_from(i).ok()?), c)))
            .collect();
        self.each(|(m, fixed)| {
            conds.iter().map(move |&(id, cond)| {
                let fixed = Fixed { cond: Some(id), ..fixed };
                (m.clone().with(cond, at), fixed)
            })
        })
    }

    /// Each spelling at each of `widths`, fixing the width where the spelling names it; as it
    /// is where the form lists none.
    pub(super) fn sized(self, widths: &[Width]) -> Self {
        if widths.is_empty() {
            return self;
        }
        let mut spelled = self.each(|(m, fixed)| {
            widths.iter().map(move |w| {
                let mut m = m.clone();
                if let Some(suffix) = &w.suffix {
                    m.word = Symbol::intern(&[m.word.as_str(), suffix].concat());
                }
                m.parts.extend(&w.qualifiers);
                let size = if w.named() { w.size } else { fixed.size };
                (m, Fixed { size, ..fixed })
            })
        });
        spelled.0.dedup();
        spelled
    }

    /// Each spelling as it is, and with `qualifier` after a `.`.
    pub(super) fn qualified(self, qualifier: Symbol) -> Self {
        self.each(|(m, fixed)| [(m.clone(), fixed), (m.with(qualifier, At::Dotted), fixed)])
    }

    fn each<I: IntoIterator<Item = (Mnemonic, Fixed)>>(self, f: impl Fn((Mnemonic, Fixed)) -> I) -> Self {
        Self(self.0.into_iter().flat_map(f).collect())
    }

    pub(super) fn into_vec(self) -> Vec<(Mnemonic, Fixed)> {
        self.0
    }
}

impl std::fmt::Display for Mnemonic {
    /// As a line writes it: `vcvt.s32.f32`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.word.as_str())?;
        self.parts.iter().try_for_each(|p| write!(f, ".{p}"))
    }
}

impl Mnemonic {
    /// `part` written at `at`.
    fn with(mut self, part: Symbol, at: At) -> Self {
        match at {
            At::Glued => self.word = Symbol::intern(&[self.word.as_str(), part.as_str()].concat()),
            At::Dotted => self.parts.push(part),
        }
        self
    }
}
