//! How tightly forms hold their operands, by the table `precedence`.

use std::sync::LazyLock;

use crate::{Assoc, INFIX, NodeKind, PRECEDENCE, Spacing, TokenKind};

/// The row of `precedence` holding the infix form.
static INFIX_ROW: LazyLock<Option<Level>> = LazyLock::new(|| NodeKind::INFIX_FORM.level());

/// How tightly a form holds its operands: its row of `precedence`, loosest first, and in the row
/// of [`NodeKind::INFIX_FORM`], its operator's row of `infix`. Higher binds tighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Level {
    row: u8,
    infix: u8,
}

impl Level {
    /// Looser than or as loose as every form's.
    pub const LOWEST: Self = Self { row: 0, infix: 0 };

    /// Tighter than or as tight as every form's: a form holding no operand at its ends.
    pub const HIGHEST: Self = Self { row: u8::MAX, infix: u8::MAX };

    /// The next tighter level, for the right operand of a left-associative operator.
    #[must_use]
    pub const fn tighter(self) -> Self {
        Self { infix: self.infix + 1, ..self }
    }

    fn at(row: usize, infix: usize) -> Self {
        let narrow = |n: usize| u8::try_from(n).expect("the grammar has fewer than 256 levels");
        Self { row: narrow(row), infix: narrow(infix) }
    }
}

/// How an infix operator binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Infix {
    pub level: Level,
    pub assoc: Assoc,
    /// How the formatter spaces the operator.
    pub around: Spacing,
}

impl NodeKind {
    /// How tightly this form holds its operands; `None` for a form `precedence` does not rank.
    #[must_use]
    pub fn level(self) -> Option<Level> {
        PRECEDENCE.iter().position(|row| row.contains(&self)).map(|row| Level::at(row, 0))
    }

    /// The level of the operand this form holds at its end: its own, or any for a form
    /// `precedence` does not rank.
    #[must_use]
    pub fn operand_level(self) -> Level {
        self.level().unwrap_or(Level::LOWEST)
    }
}

impl TokenKind {
    #[must_use]
    pub fn infix(self) -> Option<Infix> {
        let row = (*INFIX_ROW)?.row;
        INFIX.iter().enumerate().find_map(|(level, infix)| {
            let level = Level::at(usize::from(row), level);
            infix.tokens.contains(&self).then_some(Infix { level, assoc: infix.assoc, around: infix.around })
        })
    }
}
