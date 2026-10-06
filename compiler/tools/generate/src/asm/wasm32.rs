//! `vun-isa-wasm32`'s tables, from `lang.arch.wasm32`: its forms as `table::Form` values, an
//! opcode, the operands, and the values popped and pushed.

use proc_macro2::TokenStream;
use quote::quote;
use vun_syntax::literal::Literal;

use super::emit::Crate;
use super::spell::{int, option, word};
use super::{ARCH, Table};
use crate::generate::{Emit, Generator, Module, Read, Source};

pub(crate) const WASM32: Generator = Generator {
    source: ARCH,
    output: "compiler/isa/wasm32/src/generated",
    emit: Emit::Rust(emit),
    keeps: &[],
};

const CRATE: Crate = Crate {
    generator: &WASM32,
    arch: "wasm32",
    types: &[
        ("Size", "table", "Size"),
        ("Kind", "table", "Kind"),
        ("Imm", "table", "Imm"),
        ("Access", "table", "Access"),
        ("Ty", "table", "Ty"),
        ("Flow", "table", "Flow"),
        ("Control", "table", "Control"),
        ("Choice", "table", "Choice"),
        ("Feature", "table", "Feature"),
        ("Need", "table", "Need"),
    ],
    widths: &[],
    enums: &["Size", "Feature", "Yields", "Global"],
    payloads: &[
        ("Kind", "imm", &["Imm"]),
        ("Kind", "choice", &["Choice"]),
        ("Imm", "unsigned", &["u8"]),
        ("Imm", "signed", &["u8"]),
        ("Imm", "bits", &["u8"]),
        ("Need", "feature", &["Feature"]),
        ("Need", "float", &["Size"]),
    ],
};

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    CRATE.modules(&Table::read(&WASM32, CRATE.arch, sources, |_, _| None)?, form)
}

/// A form as a `table::Form` value, a builder call for each field its data sets.
fn form(c: &Crate, record: &Literal<'_>) -> Option<TokenStream> {
    let field = |name| record.get(name);
    let base = word(field("base")?)?;
    let args = field("args")?.as_list()?.iter().map(|a| {
        let (kind, access) = (c.value("Kind", a.get("kind")?)?, c.variant("Access", a.get("access")?)?);
        Some(quote! { table::Arg::new(#kind, #access) })
    });
    let args = args.collect::<Option<Vec<_>>>()?;
    let (pops, pushes) = (c.variants("Ty", field("pops")?)?, c.variants("Ty", field("pushes")?)?);
    let mut form = quote! { table::Form::new(#base, &[#(#args),*], &[#(#pops),*], &[#(#pushes),*]) };
    let tail = field("tail")?.as_list()?.iter().map(int::<u8>).collect::<Option<Vec<_>>>()?;
    if !tail.is_empty() {
        form = quote! { #form.tail(&[#(#tail),*]) };
    }
    let flow = c.variant("Flow", field("flow")?)?;
    let control = c.variant("Control", field("control")?)?;
    form = quote! { #form.flow(#flow).control(#control) };
    let requires = option(field("requires")?, |need| c.value("Need", need))?;
    Some(quote! { #form.requires(#requires) })
}
