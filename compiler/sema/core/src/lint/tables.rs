//! The `lang` values lints are read with, each variant by its handle.

use vun_diag::Severity;
use vun_items::{Item, lang};
use vun_sema_ty::Value;

/// A lint's level (`lang.lint.Level`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    Allow,
    Warn,
    Deny,
    Forbid,
    Expect,
}

impl Level {
    const ALL: [(Item, Self); 5] = [
        (lang::lint_Level::allow, Self::Allow),
        (lang::lint_Level::warn, Self::Warn),
        (lang::lint_Level::deny, Self::Deny),
        (lang::lint_Level::forbid, Self::Forbid),
        (lang::lint_Level::expect, Self::Expect),
    ];

    /// The level the variant `value` of `lang.lint.Level` is, or names where no type was
    /// expected (`.warn` in a record whose type is inferred).
    pub(super) fn of(value: &Value) -> Option<Self> {
        match value {
            Value::Name(name) => Self::ALL.iter().find(|(item, _)| item.symbol() == *name).map(|&(_, l)| l),
            _ => variant(value, &Self::ALL),
        }
    }

    /// The level the name `name` is, as [`Level::name`] spells it.
    #[must_use]
    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|(item, _)| item.name() == name).map(|&(_, level)| level)
    }

    /// Its name, the spelling [`Level::of_name`] reads.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.item().name()
    }

    /// Every name a level is written as, in the order of [`Level`].
    pub fn names() -> impl Iterator<Item = &'static str> {
        Self::ALL.iter().map(|(item, _)| item.name())
    }

    /// The level itself, or `warn` where it is `deny` or `forbid`: how a dependency's lint
    /// reports while a dependent is built (spec 10, section 4).
    #[must_use]
    pub fn capped(self) -> Self {
        match self {
            Self::Deny | Self::Forbid => Self::Warn,
            other => other,
        }
    }

    /// The variant of `lang.lint.Level` it is.
    pub(super) fn item(self) -> Item {
        Self::ALL.iter().find(|&&(_, level)| level == self).map_or(lang::lint_Level::allow, |&(item, _)| item)
    }

    /// How a report at this level is shown; `None` where it is not.
    pub(super) fn severity(self) -> Option<Severity> {
        match self {
            Self::Allow => None,
            Self::Warn | Self::Expect => Some(Severity::Warning),
            Self::Deny | Self::Forbid => Some(Severity::Error),
        }
    }
}

/// What a declaration binds (`lang.lint.Binds`), as compile-time evaluation knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Binds {
    Type,
    Value,
    Field,
    Variant,
}

impl Binds {
    /// The variant of `lang.lint.Binds` it is.
    pub(super) fn item(self) -> Item {
        match self {
            Self::Type => lang::lint_Binds::r#type,
            Self::Value => lang::lint_Binds::value,
            Self::Field => lang::lint_Binds::field,
            Self::Variant => lang::lint_Binds::variant,
        }
    }
}

/// What `value`, a variant, stands for among `table`, by the variant's handle.
fn variant<T: Copy>(value: &Value, table: &[(Item, T)]) -> Option<T> {
    let Value::Variant { name, .. } = value else { return None };
    table.iter().find(|(item, _)| item.symbol() == *name).map(|&(_, t)| t)
}
