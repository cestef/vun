//! `vun-asm-x86_64`'s tables, from `lang.arch.x86_64`: its forms as `table::Form` values.

use proc_macro2::TokenStream;
use quote::quote;
use vun_syntax::literal::Literal;

use super::emit::Crate;
use super::spell::{implicit, int, option};
use super::{ARCH, Table};
use crate::generate::{Generator, Module, Read, Source};
use crate::names::pascal;

pub(crate) const X86_64: Generator =
    Generator { source: ARCH, output: "compiler/isa/x86_64/src/generated", emit };

const CRATE: Crate = Crate {
    generator: &X86_64,
    arch: "x86_64",
    types: &[
        ("Size", "reg", "Size"),
        ("Reg", "reg", "Reg"),
        ("Kind", "table", "Kind"),
        ("Access", "table", "Access"),
        ("Prefix", "table", "Prefix"),
        ("W", "table", "W"),
        ("Feature", "table", "Feature"),
        ("Need", "table", "Need"),
    ],
    widths: &["b8", "b16", "b32", "b64"],
    enums: &["Feature"],
    payloads: &[
        ("Kind", "rn", &["Size"]),
        ("Kind", "rmn", &["Size"]),
        ("Kind", "xmn", &["Size"]),
        ("Need", "feature", &["Feature"]),
        ("Need", "float", &["Size"]),
    ],
};

/// The first byte of a two-byte opcode.
const ESCAPE: i128 = 0x0f;

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    CRATE.modules(&Table::read(&X86_64, CRATE.arch, sources, fill)?, form)
}

/// A hole of a family's form, filled from `member`: its own byte, the member's opcode with bits
/// set, or the member's opcode extension.
fn fill<'s>(value: &Literal<'s>, member: &Literal<'s>) -> Option<Literal<'s>> {
    let field = |name| member.get(name).and_then(|v| if let Literal::Int(n) = v { Some(*n) } else { None });
    match value {
        Literal::Tagged("fixed", payload) => payload.first().cloned(),
        Literal::Tagged("opcode", payload) => match payload.first() {
            Some(Literal::Int(bits)) => Some(Literal::Int(field("opcode")? | bits)),
            _ => None,
        },
        Literal::Name("digit") => field("digit").map(Literal::Int),
        _ => None,
    }
}

/// A form as a `table::Form` value.
fn form(c: &Crate, record: &Literal<'_>) -> Option<TokenStream> {
    let field = |name| record.get(name);
    let slots = field("operands")?.as_list()?.iter().map(|o| {
        let (kind, access) = (c.value("Kind", o.get("kind")?)?, c.variant("Access", o.get("access")?)?);
        Some(quote! { table::Slot { kind: #kind, access: #access } })
    });
    let slots = slots.collect::<Option<Vec<_>>>()?;
    let sizes = c.variants("Size", field("sizes")?)?;
    let opcode = match field("opcode")?.as_list()? {
        [a] => {
            let a = int::<u8>(a)?;
            quote! { table::Opcode::one(#a) }
        }
        [Literal::Int(ESCAPE), b] => {
            let b = int::<u8>(b)?;
            quote! { table::Opcode::two(#b) }
        }
        _ => return None,
    };
    let layout = match field("layout")? {
        Literal::Tagged("reg_rm", payload) => {
            let at = |f| int::<u8>(payload.first()?.get(f)?);
            let (reg, rm) = (at("reg")?, at("rm")?);
            quote! { table::Layout::RegRm { reg: #reg, rm: #rm } }
        }
        Literal::Tagged("digit", payload) => {
            let at = |f| int::<u8>(payload.first()?.get(f)?);
            let (digit, rm) = (at("digit")?, at("rm")?);
            quote! { table::Layout::Digit { digit: #digit, rm: #rm } }
        }
        Literal::Tagged("plus_reg", payload) => {
            let at = int::<u8>(payload.first()?)?;
            quote! { table::Layout::PlusReg(#at) }
        }
        Literal::Name("bare") => quote! { table::Layout::Bare },
        _ => return None,
    };
    let byte = option(field("byte")?, int::<u8>)?;
    let (prefix, w) = (c.variant("Prefix", field("prefix")?)?, c.variant("W", field("w")?)?);
    let name = option(field("name")?, |n| n.as_str().map(|s| quote! { #s }))?;
    let flags = option(field("flags")?, |a| c.variant("Access", a))?;
    let requires = option(field("requires")?, |n| c.value("Need", n))?;
    let implicit = implicit(field("reads")?.as_list()?, field("writes")?.as_list()?);
    let implicit = implicit.iter().map(|r| pascal(r));
    let (reads, writes) = (c.variants("Reg", field("reads")?)?, c.variants("Reg", field("writes")?)?);
    Some(quote! {
        table::Form {
            slots: &[#(#slots),*],
            sizes: table::Sizes::of(&[#(#sizes),*]),
            prefix: #prefix,
            opcode: #opcode,
            byte: #byte,
            w: #w,
            layout: #layout,
            name: #name,
            reads: &[#(#reads),*],
            writes: &[#(#writes),*],
            implicit: &[#(reg::Reg::#implicit),*],
            flags: #flags,
            requires: #requires,
        }
    })
}
