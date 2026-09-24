//! What of `lang` the bindings are written with, each by its handle in `vun-items`, and the
//! syntax the grammar's tables give it: `?` and `*` by `type_prefixes`, `unsafe` by `markers`,
//! the type of conditions by `forms`. Paths are fully qualified, so bindings read the same under
//! any prelude.

use vun_desugar::{FORMS, Form, MARKERS, TYPE_PREFIXES};
use vun_grammar::TokenKind;
use vun_items::{Item, lang};
use vun_syntax::make::Expr;

/// `lang.int.unsigned(bits)` or `lang.int.signed(bits)`.
pub(crate) fn integer(signed: bool, bits: u64) -> Expr {
    let width = if signed { lang::int::signed } else { lang::int::unsigned };
    Expr::call(Expr::decl(width), vec![Expr::Int(i128::from(bits))])
}

pub(crate) fn float(bits: u64) -> Option<Expr> {
    match bits {
        32 => Some(Expr::decl(lang::f32)),
        64 => Some(Expr::decl(lang::f64)),
        _ => None,
    }
}

/// The type of conditions, C's `_Bool`.
pub(crate) fn condition() -> Expr {
    let (_, item) = FORMS.iter().find(|(f, _)| *f == Form::Condition).expect("the table names conditions");
    Expr::decl(*item)
}

/// `?inner`
pub(crate) fn option(inner: Expr) -> Expr {
    Expr::prefix(token(TYPE_PREFIXES, lang::Option), inner)
}

/// `*inner`
pub(crate) fn pointer(inner: Expr) -> Expr {
    Expr::prefix(token(TYPE_PREFIXES, lang::mem::Ptr), inner)
}

/// `unsafe inner`
pub(crate) fn unchecked(inner: Expr) -> Expr {
    Expr::prefix(token(MARKERS, lang::safety::r#unsafe), inner)
}

/// `#[lang.abi.c] inner`
pub(crate) fn c_abi(inner: Expr) -> Expr {
    Expr::Attributed(vec![Expr::decl(lang::abi::c)], Box::new(inner))
}

/// `lang.link.symbol("name")`
pub(crate) fn symbol(name: &str) -> Expr {
    Expr::call(Expr::decl(lang::link::symbol), vec![Expr::String(name.to_owned())])
}

pub(crate) fn abi_c() -> Expr {
    Expr::decl(lang::abi::c)
}

pub(crate) fn layout_c() -> Expr {
    Expr::decl(lang::layout::c)
}

/// `lang.layout.align(bytes)`
pub(crate) fn align(bytes: u64) -> Expr {
    Expr::call(Expr::decl(lang::layout::align), vec![Expr::Int(i128::from(bytes))])
}

/// The token `table` writes `item` with.
fn token(table: &[(TokenKind, Item)], item: Item) -> TokenKind {
    table.iter().find(|(_, i)| *i == item).map(|(t, _)| *t).expect("the table writes the item")
}
