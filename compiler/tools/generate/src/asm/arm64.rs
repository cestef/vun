//! `vun-asm-arm64`'s tables, from `lang.arch.arm64`: its forms as `table::Form` values.

use proc_macro2::TokenStream;
use quote::quote;
use vun_syntax::literal::Literal;

use super::emit::{Crate, int, word};
use super::{ARCH, Table};
use crate::generate::{Generator, Module, Read, Source};

pub(crate) const ARM64: Generator =
    Generator { source: ARCH, output: "compiler/back/arm64/src/generated", emit };

const CRATE: Crate = Crate {
    generator: &ARM64,
    arch: "arm64",
    types: &[
        ("Size", "reg", "Size"),
        ("Reg", "reg", "Reg"),
        ("Kind", "table", "Kind"),
        ("Access", "table", "Access"),
        ("R31", "table", "R31"),
        ("Width", "table", "Width"),
        ("Imm", "table", "Imm"),
        ("Mode", "table", "Mode"),
        ("Shifts", "table", "Shifts"),
        ("Condition", "table", "Cond"),
        ("Branch", "table", "Branch"),
    ],
    widths: &["w", "x"],
    payloads: &[
        ("Kind", "gpr", &["R31", "Width"]),
        ("Kind", "fpr", &["Width"]),
        ("Kind", "imm", &["Imm"]),
        ("Kind", "shifted", &["Shifts"]),
        ("Kind", "mem", &["Mode", "Width"]),
        ("Kind", "cond", &["Condition"]),
        ("Kind", "target", &["Branch"]),
        ("Width", "is", &["Size"]),
    ],
};

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    CRATE.modules(&Table::read(&ARM64, CRATE.arch, sources, fill)?, form)
}

/// A hole of a family's form, filled from `member`: its own base word, or the member's bits with
/// these set.
fn fill<'s>(value: &Literal<'s>, member: &Literal<'s>) -> Option<Literal<'s>> {
    match value {
        Literal::Tagged("fixed", payload) => payload.first().cloned(),
        Literal::Tagged("or", payload) => match (payload.first(), member.get("bits")) {
            (Some(Literal::Int(bits)), Some(Literal::Int(own))) => Some(Literal::Int(own | bits)),
            _ => None,
        },
        _ => None,
    }
}

/// A form as a `table::Form` value, built by the table's constructors, a builder call for each
/// field its data sets.
fn form(c: &Crate, record: &Literal<'_>) -> Option<TokenStream> {
    let field = |name| record.get(name);
    let base = word(field("base")?)?;
    let sizes = field("sizes")?.as_list()?.iter().map(|s| {
        let (size, bits) = (c.variant("Size", s.get("size")?)?, word(s.get("bits")?)?);
        let sized = quote! { table::Sized::new(#size, #bits) };
        Some(match s.get("suffix")?.as_str()? {
            "" => sized,
            suffix => quote! { #sized.suffix(#suffix) },
        })
    });
    let sizes = sizes.collect::<Option<Vec<_>>>()?;
    let args = field("args")?.as_list()?.iter().map(|a| {
        let (kind, at) = (c.value("Kind", a.get("kind")?)?, int::<u8>(a.get("at")?)?);
        let access = c.variant("Access", a.get("access")?)?;
        let arg = quote! { table::Arg::new(#kind, #at, #access) };
        Some(match a.get("again")? {
            Literal::Tagged("some", again) => {
                let again = int::<u8>(again.first()?)?;
                quote! { #arg.again(#again) }
            }
            _ => arg,
        })
    });
    let args = args.collect::<Option<Vec<_>>>()?;
    let mut form = quote! { table::Form::new(#base, &[#(#sizes),*], &[#(#args),*]) };
    if let Literal::Tagged("some", name) = field("name")? {
        let name = name.first()?.as_str()?;
        form = quote! { #form.named(#name) };
    }
    let (reads, writes) = (c.variants("Reg", field("reads")?)?, c.variants("Reg", field("writes")?)?);
    if !reads.is_empty() {
        form = quote! { #form.reads(&[#(#reads),*]) };
    }
    if !writes.is_empty() {
        form = quote! { #form.writes(&[#(#writes),*]) };
    }
    if let Literal::Tagged("some", flags) = field("flags")? {
        let flags = c.variant("Access", flags.first()?)?;
        form = quote! { #form.flags(#flags) };
    }
    Some(form)
}
