//! Thumb's table, `lang.arch.thumb`: no encodings, so a form is its operands' kinds and what each
//! does, the widths it is defined at, its mnemonic where not its instruction's, and the special
//! registers it reads and writes; the table's plain enums are generated as they are.

use proc_macro2::TokenStream;
use quote::quote;
use vun_syntax::literal::Literal;

use super::emit::Crate;
use super::spell::option;
use super::{ARCH, Table};
use crate::generate::{Generator, Module, Read, Source};

pub(crate) const THUMB: Generator =
    Generator { source: ARCH, output: "compiler/base/thumb/src/generated", emit };

const CRATE: Crate = Crate {
    generator: &THUMB,
    arch: "thumb",
    types: &[
        ("Size", "reg", "Size"),
        ("Reg", "reg", "Reg"),
        ("Special", "reg", "Special"),
        ("Kind", "table", "Kind"),
        ("Access", "table", "Access"),
    ],
    widths: &[],
    enums: &["Size", "Special", "Mask", "Barrier", "Kind", "Access"],
    payloads: &[],
};

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    CRATE.modules(&Table::read(&THUMB, CRATE.arch, sources, fill)?, form)
}

/// A family's forms have no holes: its members differ by name alone.
fn fill<'s>(_: &Literal<'s>, _: &Literal<'s>) -> Option<Literal<'s>> {
    None
}

/// A form as a `table::Form` value.
fn form(c: &Crate, record: &Literal<'_>) -> Option<TokenStream> {
    let field = |name| record.get(name);
    let slots = field("operands")?.as_list()?.iter().map(|o| {
        let (kind, access) = (c.variant("Kind", o.get("kind")?)?, c.variant("Access", o.get("access")?)?);
        Some(quote! { table::Slot { kind: #kind, access: #access } })
    });
    let slots = slots.collect::<Option<Vec<_>>>()?;
    let sizes = c.variants("Size", field("sizes")?)?;
    let name = option(field("name")?, |n| n.as_str().map(|s| quote! { #s }))?;
    let (reads, writes) = (c.variants("Special", field("reads")?)?, c.variants("Special", field("writes")?)?);
    Some(quote! {
        table::Form {
            slots: &[#(#slots),*],
            sizes: &[#(#sizes),*],
            name: #name,
            reads: &[#(#reads),*],
            writes: &[#(#writes),*],
        }
    })
}
