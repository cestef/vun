//! `vun-asm-x86_64`'s tables, from `lang.arch.x86_64`: the general registers with their names by
//! width, the `Op` enum, and each instruction's forms as `table::Form` values.

use proc_macro2::{Literal as Number, TokenStream};
use quote::quote;
use vun_diag::{DiagId, Msg};
use vun_syntax::literal::Literal;

use super::{ARCH, Table};
use crate::generate::{Access, Generator, Module, Read, Source};
use crate::names::{constant, pascal, raw};

pub(crate) const X86_64: Generator =
    Generator { source: ARCH, output: "compiler/back/x86_64/src/generated", emit };

const NAME: &str = "x86_64";
/// The general registers, and their names by width.
const REG: &str = "Reg";
const NAMES: &str = "names";
const WIDTHS: [&str; 4] = ["b8", "b16", "b32", "b64"];
/// The module of the `Op` enum.
const OPS: &str = "ops";
/// The first byte of a two-byte opcode.
const ESCAPE: i128 = 0x0f;

const SHAPE: DiagId = DiagId::new("generate.asm.shape");

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let table = Table::read(&X86_64, NAME, sources, fill)?;
    let shape = |what: Msg| X86_64.error(SHAPE, NAME, what);
    let registers = registers(&table);
    let mut modules = vec![
        registers
            .ok_or_else(|| shape(Msg::new().text("declares its registers or their names unreadably")))?,
    ];
    let mut variants = Vec::new();
    for inst in &table.instructions {
        let forms = inst.forms.iter().map(form).collect::<Option<Vec<_>>>();
        let forms = forms.ok_or_else(|| {
            shape(Msg::new().text("declares a form of ").code(inst.name).text(" unreadably"))
        })?;
        let (name, count, doc) = (constant(inst.name), forms.len(), &inst.doc);
        let items = quote! {
            use crate::{reg, table};
            #(#[doc = #doc])*
            pub(super) const #name: [table::Form; #count] = [#(#forms),*];
        };
        modules.push(Module { name: inst.name.to_owned(), access: Access::Sibling, items });
        variants.push((inst.name, doc.clone()));
    }
    modules.push(Module::sibling(OPS.to_owned(), &ops(&variants)));
    Ok(modules)
}

/// The `Op` enum: each instruction, its mnemonic and its forms.
fn ops(variants: &[(&str, Vec<String>)]) -> TokenStream {
    let names: Vec<_> = variants.iter().map(|(n, _)| pascal(n)).collect();
    let docs = variants.iter().map(|(_, d)| d);
    let mnemonics = variants.iter().map(|(n, _)| *n);
    let consts = variants.iter().map(|(n, _)| {
        let (module, name) = (raw(n), constant(n));
        quote! { #module::#name }
    });
    quote! {
        use crate::table::Form;
        /// An instruction, by its mnemonic; one taking a condition names the family (`J` for `jcc`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Op { #(#(#[doc = #docs])* #names,)* }
        impl Op {
            pub const ALL: &[Self] = &[#(Self::#names),*];
            /// The mnemonic, or its stem for an instruction taking a condition.
            #[must_use]
            pub const fn mnemonic(self) -> &'static str {
                match self { #(Self::#names => #mnemonics,)* }
            }
            /// Its forms, in the order they are tried: shorter encodings first.
            #[must_use]
            pub const fn forms(self) -> &'static [Form] {
                match self { #(Self::#names => &#consts,)* }
            }
        }
    }
}

/// The `Reg` enum in encoding order, and each register's names by width.
fn registers(table: &Table<'_, '_>) -> Option<Module> {
    let regs: Vec<&str> = table.own(REG)?.value.members()?.iter().map(|d| d.name).collect();
    let names = &table.own(NAMES)?.value;
    let rows = regs.iter().map(|r| {
        let entry = names.get(r)?;
        let row =
            WIDTHS.iter().map(|w| entry.get(w)?.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>()?;
        Some(quote! { [#(#row),*] })
    });
    let rows = rows.collect::<Option<Vec<_>>>()?;
    let (variants, count) = (regs.iter().map(|r| pascal(r)).collect::<Vec<_>>(), regs.len());
    let items = quote! {
        /// A general register, by its encoding.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Reg { #(#variants,)* }
        impl Reg {
            pub const ALL: [Self; #count] = [#(Self::#variants),*];
        }
        /// Each register's name at each width, as Intel syntax writes it.
        pub(crate) const NAMES: [[&str; 4]; #count] = [#(#rows),*];
    };
    Some(Module { name: REG.to_ascii_lowercase(), access: Access::Glob, items })
}

/// A hole of a family's form, filled from `member`: its own byte, the member's opcode with bits
/// set, or the member's opcode extension.
fn fill<'s>(value: &Literal<'s>, member: &Literal<'s>) -> Option<Literal<'s>> {
    let int = |field| member.get(field).and_then(|v| if let Literal::Int(n) = v { Some(*n) } else { None });
    match value {
        Literal::Tagged("is", payload) => payload.first().cloned(),
        Literal::Tagged("opcode", payload) => match payload.first() {
            Some(Literal::Int(bits)) => Some(Literal::Int(int("opcode")? | bits)),
            _ => None,
        },
        Literal::Name("digit") => int("digit").map(Literal::Int),
        _ => None,
    }
}

/// A form as a `table::Form` value.
fn form(record: &Literal<'_>) -> Option<TokenStream> {
    let field = |name| record.get(name);
    let slots = field("operands")?.as_list()?.iter().map(|o| {
        let (kind, access) = (kind(o.get("kind")?)?, variant("Access", o.get("access")?)?);
        Some(quote! { table::Slot { kind: #kind, access: #access } })
    });
    let slots = slots.collect::<Option<Vec<_>>>()?;
    let sizes = names("Size", field("sizes")?)?;
    let opcode = match field("opcode")?.as_list()? {
        [Literal::Int(a)] => {
            let a = byte(*a)?;
            quote! { table::Opcode::one(#a) }
        }
        [Literal::Int(ESCAPE), Literal::Int(b)] => {
            let b = byte(*b)?;
            quote! { table::Opcode::two(#b) }
        }
        _ => return None,
    };
    let layout = match field("layout")? {
        Literal::Tagged("reg_rm", payload) => {
            let (reg, rm) = (byte_of(payload.first()?.get("reg")?)?, byte_of(payload.first()?.get("rm")?)?);
            quote! { table::Layout::RegRm { reg: #reg, rm: #rm } }
        }
        Literal::Tagged("digit", payload) => {
            let (digit, rm) =
                (byte_of(payload.first()?.get("digit")?)?, byte_of(payload.first()?.get("rm")?)?);
            quote! { table::Layout::Digit { digit: #digit, rm: #rm } }
        }
        Literal::Tagged("plus_reg", payload) => {
            let at = byte_of(payload.first()?)?;
            quote! { table::Layout::PlusReg(#at) }
        }
        Literal::Name("bare") => quote! { table::Layout::Bare },
        _ => return None,
    };
    let byte = option(field("byte")?, byte_of)?;
    let (prefix, w) = (variant("Prefix", field("prefix")?)?, variant("W", field("w")?)?);
    let name = option(field("name")?, |n| n.as_str().map(|s| quote! { #s }))?;
    let flags = option(field("flags")?, |a| variant("Access", a))?;
    let (reads, writes) = (field("reads")?.as_list()?, field("writes")?.as_list()?);
    let mut implicit: Vec<&str> = Vec::new();
    for reg in reads.iter().chain(writes).filter_map(Literal::as_name) {
        if !implicit.contains(&reg) {
            implicit.push(reg);
        }
    }
    let implicit = implicit.iter().map(|r| pascal(r));
    let (reads, writes) = (names("Reg", field("reads")?)?, names("Reg", field("writes")?)?);
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
        }
    })
}

/// An operand kind, `Kind::Rmn(Size::B32)` for `.rmn(.b32)`.
fn kind(value: &Literal<'_>) -> Option<TokenStream> {
    match value {
        Literal::Name(name) => {
            let name = pascal(name);
            Some(quote! { table::Kind::#name })
        }
        Literal::Tagged(tag, payload) => {
            let (tag, size) = (pascal(tag), variant("Size", payload.first()?)?);
            Some(quote! { table::Kind::#tag(#size) })
        }
        _ => None,
    }
}

/// The variant of the Rust type `ty` a `.name` value names, by its path from the crate's root.
fn variant(ty: &str, value: &Literal<'_>) -> Option<TokenStream> {
    let module = raw(if matches!(ty, "Reg" | "Size") { "reg" } else { "table" });
    let (ty, name) = (pascal(ty), pascal(value.as_name()?));
    Some(quote! { #module::#ty::#name })
}

/// A list of `.name` values as variants of `ty`.
fn names(ty: &str, value: &Literal<'_>) -> Option<Vec<TokenStream>> {
    value.as_list()?.iter().map(|v| variant(ty, v)).collect()
}

/// `.some(v)` as `Some(..)` of `v` read by `read`, `.none` as `None`.
fn option<'s>(
    value: &Literal<'s>,
    read: impl Fn(&Literal<'s>) -> Option<TokenStream>,
) -> Option<TokenStream> {
    match value {
        Literal::Name("none") => Some(quote! { None }),
        Literal::Tagged("some", payload) => {
            let inner = read(payload.first()?)?;
            Some(quote! { Some(#inner) })
        }
        _ => None,
    }
}

fn byte(n: i128) -> Option<Number> {
    u8::try_from(n).ok().map(Number::u8_unsuffixed)
}

fn byte_of(value: &Literal<'_>) -> Option<TokenStream> {
    let Literal::Int(n) = value else { return None };
    byte(*n).map(|b| quote! { #b })
}
