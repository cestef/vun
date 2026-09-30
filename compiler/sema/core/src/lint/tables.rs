//! The `lang` values lints are read with, each variant by its handle.

use vun_diag::{Safety, Severity};
use vun_items::{Item, lang};
use vun_sema_ty::Value;

/// A lint's level (`lang.lint.Level`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Level {
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

/// How safe applying a fix is, from the variant `value` of `lang.diag.Safety`.
pub(super) fn safety(value: &Value) -> Option<Safety> {
    const ALL: [(Item, Safety); 4] = [
        (lang::diag_Safety::safe, Safety::Safe),
        (lang::diag_Safety::r#unsafe, Safety::Unsafe),
        (lang::diag_Safety::placeholder, Safety::Placeholder),
        (lang::diag_Safety::display_only, Safety::DisplayOnly),
    ];
    variant(value, &ALL)
}

/// What `value`, a variant, stands for among `table`, by the variant's handle.
fn variant<T: Copy>(value: &Value, table: &[(Item, T)]) -> Option<T> {
    let Value::Variant { name, .. } = value else { return None };
    table.iter().find(|(item, _)| item.symbol() == *name).map(|&(_, t)| t)
}
