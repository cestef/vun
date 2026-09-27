//! What every architecture's assembler table is generated as: its general registers with their
//! names by width, a module per instruction holding its forms, and the `Op` enum over them; and
//! the Rust spelling of the table's values.

use proc_macro2::{Literal as Number, TokenStream};
use quote::quote;
use vun_diag::{DiagId, Msg};
use vun_syntax::literal::Literal;

use super::{Instruction, Table};
use crate::generate::{Access, Generator, Module, Read};
use crate::names::{constant, pascal, raw};

/// The general registers, and their names by width.
const REG: &str = "Reg";
const NAMES: &str = "names";
/// The conditions, in encoding order.
const COND: &str = "Cond";
/// The classes of registers an operand names.
const CLASS: &str = "Class";
/// The enum of the kinds of named choice, which an architecture declares where it has any.
const CHOICE: &str = "Choice";
/// The modules of the `Op` enum, its mnemonics and its forms.
const OPS: &str = "ops";
const MNEMONICS: &str = "mnemonics";
const FORMS: &str = "forms";

const SHAPE: DiagId = DiagId::new("generate.asm.shape");

/// An architecture's assembler crate: where each type of its table lives there, by the Vun name
/// the table gives it, and the widths its registers are named at.
pub(super) struct Crate {
    pub(super) generator: &'static Generator,
    pub(super) arch: &'static str,
    /// Each Vun type of the table, its crate module and its Rust name.
    pub(super) types: &'static [(&'static str, &'static str, &'static str)],
    /// The widths a register's name is given at, by `names`; none where the table has no names,
    /// each register named as its variant.
    pub(super) widths: &'static [&'static str],
    /// The payload types of the tagged variants the table uses, by type and variant.
    pub(super) payloads: &'static [(&'static str, &'static str, &'static [&'static str])],
    /// The table's enums generated as they are, each in a module of its own, where the crate
    /// does not write them itself.
    pub(super) enums: &'static [&'static str],
}

impl Crate {
    /// The modules of `table`, each instruction's forms spelled by `form`.
    pub(super) fn modules(
        &self,
        table: &Table<'_, '_>,
        form: impl Fn(&Self, &Literal<'_>) -> Option<TokenStream>,
    ) -> Read<Vec<Module>> {
        let shape = |what: Msg| self.generator.error(SHAPE, self.arch, what);
        let registers = self.registers(table);
        let conditions = Self::conditions(table);
        let classes = Self::classes(table);
        let mut modules = vec![
            registers.ok_or_else(|| shape(Msg::new().text("declares its registers unreadably")))?,
            conditions.ok_or_else(|| shape(Msg::new().text("declares its conditions unreadably")))?,
            classes.ok_or_else(|| shape(Msg::new().text("declares its register classes unreadably")))?,
        ];
        modules.extend(Self::choices(table));
        for &name in self.enums {
            let rust = raw(name);
            let enumeration = Self::enumeration(table, name, &quote! { #rust });
            let enumeration = enumeration
                .ok_or_else(|| shape(Msg::new().text("declares ").code(name).text(" unreadably")))?;
            let doc = table.own(name).map(|d| crate::names::docs(&d.doc)).unwrap_or_default();
            let members = table.own(name).and_then(|d| d.value.members()).unwrap_or_default();
            let (names, count) = (members.iter().map(|m| m.name), members.len());
            let items = quote! {
                #(#[doc = #doc])* #enumeration
                impl #rust {
                    /// Its name, as the table writes it.
                    #[must_use]
                    pub const fn name(self) -> &'static str {
                        const NAMES: [&str; #count] = [#(#names),*];
                        NAMES[self as usize]
                    }
                }
            };
            modules.push(Module { name: name.to_ascii_lowercase(), access: Access::Glob, items });
        }
        for inst in &table.instructions {
            let forms = inst.forms.iter().map(|f| form(self, f)).collect::<Option<Vec<_>>>();
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
        }
        modules.extend(Self::ops(&table.instructions));
        Ok(modules)
    }

    /// The `Op` enum of each instruction, and its mnemonics and forms, each in a module.
    fn ops(instructions: &[Instruction<'_>]) -> [Module; 3] {
        let names: Vec<_> = instructions.iter().map(|i| pascal(i.name)).collect();
        let docs = instructions.iter().map(|i| &i.doc);
        let mnemonics = instructions.iter().map(|i| i.mnemonic.as_deref().unwrap_or(i.name));
        let consts = instructions.iter().map(|i| {
            let (module, name) = (raw(i.name), constant(i.name));
            quote! { super::#module::#name }
        });
        let ops = quote! {
            /// An instruction, by its declaration in the architecture's table.
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum Op { #(#(#[doc = #docs])* #names,)* }
            impl Op {
                pub const ALL: &[Self] = &[#(Self::#names),*];
            }
        };
        let count = instructions.len();
        let mnemonic = quote! {
            use super::Op;
            /// The mnemonic of each instruction, in the order of `Op`.
            const MNEMONICS: [&str; #count] = [#(#mnemonics),*];
            impl Op {
                /// The mnemonic its forms are written with, unless a form names its own.
                #[must_use]
                pub const fn mnemonic(self) -> &'static str {
                    MNEMONICS[self as usize]
                }
            }
        };
        let forms = quote! {
            use super::Op;
            use crate::table::Form;
            /// The forms of each instruction, in the order of `Op`.
            const FORMS: [&[Form]; #count] = [#(&#consts),*];
            impl Op {
                /// Its forms, in the order they are tried.
                #[must_use]
                pub const fn forms(self) -> &'static [Form] {
                    FORMS[self as usize]
                }
            }
        };
        [
            Module { name: OPS.to_owned(), access: Access::Glob, items: ops },
            Module { name: MNEMONICS.to_owned(), access: Access::Impl, items: mnemonic },
            Module { name: FORMS.to_owned(), access: Access::Impl, items: forms },
        ]
    }

    /// The `Reg` enum in encoding order, with its doc lines, and each register's names by width.
    fn registers(&self, table: &Table<'_, '_>) -> Option<Module> {
        let regs = table.own(REG)?.value.members()?;
        let names = table.own(NAMES).map(|d| &d.value);
        let widths = self.widths;
        let rows = regs.iter().map(|r| {
            let Some(names) = names else {
                let name = r.name;
                return Some(quote! { [#name] });
            };
            let entry = names.get(r.name)?;
            let row = widths.iter().map(|w| entry.get(w)?.as_str().map(str::to_owned));
            let row = row.collect::<Option<Vec<_>>>()?;
            Some(quote! { [#(#row),*] })
        });
        let rows = rows.collect::<Option<Vec<_>>>()?;
        let variants: Vec<_> = regs.iter().map(|r| pascal(r.name)).collect();
        let docs = regs.iter().map(|r| crate::names::docs(&r.doc));
        let (count, width_count) = (regs.len(), if names.is_some() { widths.len() } else { 1 });
        let items = quote! {
            /// A general register, by its encoding.
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub enum Reg { #(#(#[doc = #docs])* #variants,)* }
            impl Reg {
                pub const ALL: [Self; #count] = [#(Self::#variants),*];
            }
            /// Each register's name at each width, narrowest first.
            pub(crate) const NAMES: [[&str; #width_count]; #count] = [#(#rows),*];
        };
        Some(Module { name: REG.to_ascii_lowercase(), access: Access::Glob, items })
    }

    /// The `Cc` enum of the conditions in encoding order, and each one's name as mnemonics write
    /// it.
    fn conditions(table: &Table<'_, '_>) -> Option<Module> {
        let conds = table.own(COND)?.value.members()?;
        let names = conds.iter().map(|c| c.name);
        let count = conds.len();
        let enumeration = Self::enumeration(table, COND, &quote! { Cc })?;
        let items = quote! {
            /// A condition, by its encoding.
            #enumeration
            /// Each condition's name, in encoding order.
            pub(crate) const CONDITIONS: [&str; #count] = [#(#names),*];
        };
        Some(Module { name: COND.to_ascii_lowercase(), access: Access::Glob, items })
    }

    /// The `Class` enum of the register classes, in the order an operand's class is numbered.
    fn classes(table: &Table<'_, '_>) -> Option<Module> {
        let enumeration = Self::enumeration(table, CLASS, &quote! { Class })?;
        let items = quote! {
            /// A class of registers.
            #enumeration
        };
        Some(Module { name: CLASS.to_ascii_lowercase(), access: Access::Glob, items })
    }

    /// The `Choice` enum of the kinds of named choice, in the order a block's choice names its
    /// kind; none where the table declares none.
    fn choices(table: &Table<'_, '_>) -> Option<Module> {
        let enumeration = Self::enumeration(table, CHOICE, &quote! { Choice })?;
        let items = quote! {
            /// A kind of named choice a position takes.
            #enumeration
        };
        Some(Module { name: CHOICE.to_ascii_lowercase(), access: Access::Glob, items })
    }

    /// The enum `rust` of the variants of the table's enum `name`, in their order, with `ALL`.
    fn enumeration(table: &Table<'_, '_>, name: &str, rust: &TokenStream) -> Option<TokenStream> {
        let members = table.own(name)?.value.members()?;
        let variants: Vec<_> = members.iter().map(|m| pascal(m.name)).collect();
        let docs = members.iter().map(|m| crate::names::docs(&m.doc));
        let count = members.len();
        Some(quote! {
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum #rust { #(#(#[doc = #docs])* #variants,)* }
            impl #rust {
                pub const ALL: [Self; #count] = [#(Self::#variants),*];
            }
        })
    }

    /// The variant a `.name` value names of the Vun type `ty`, by its path from the crate root.
    pub(super) fn variant(&self, ty: &str, value: &Literal<'_>) -> Option<TokenStream> {
        let &(_, module, rust) = self.types.iter().find(|(vun, _, _)| *vun == ty)?;
        let (module, rust, name) = (raw(module), raw(rust), pascal(value.as_name()?));
        Some(quote! { #module::#rust::#name })
    }

    /// A list of `.name` values as variants of `ty`.
    pub(super) fn variants(&self, ty: &str, value: &Literal<'_>) -> Option<Vec<TokenStream>> {
        value.as_list()?.iter().map(|v| self.variant(ty, v)).collect()
    }

    /// A tagged value of the Vun type `ty` whose payloads are of the types `payloads`, as the
    /// Rust tuple variant: `.gpr(.zr, .inst)` is `table::Kind::Gpr(table::R31::Zr, ..)`. A
    /// payload type naming itself as `Self` is read by `nested`.
    pub(super) fn tuple(
        &self,
        ty: &str,
        tag: &str,
        payload: &[Literal<'_>],
        types: &[&str],
    ) -> Option<TokenStream> {
        let &(_, module, rust) = self.types.iter().find(|(vun, _, _)| *vun == ty)?;
        let (module, rust, tag) = (raw(module), raw(rust), pascal(tag));
        let args = payload.iter().zip(types).map(|(v, t)| self.value(t, v)).collect::<Option<Vec<_>>>()?;
        (args.len() == types.len()).then(|| quote! { #module::#rust::#tag(#(#args),*) })
    }

    /// A value of the Vun type `ty`: a `.name`, or a tagged one whose payloads are its own type's,
    /// as `Width.is(Size)` is.
    pub(super) fn value(&self, ty: &str, value: &Literal<'_>) -> Option<TokenStream> {
        match value {
            Literal::Name(_) => self.variant(ty, value),
            Literal::Tagged(tag, payload) => {
                let types = self.payloads(ty, tag)?;
                self.tuple(ty, tag, payload, types)
            }
            Literal::Int(n) => u32::try_from(*n).ok().map(|n| {
                let n = Number::u32_unsuffixed(n);
                quote! { #n }
            }),
            _ => None,
        }
    }

    /// The payload types of the variant `tag` of `ty`, as the crate's types declare them.
    fn payloads(&self, ty: &str, tag: &str) -> Option<&'static [&'static str]> {
        self.payloads.iter().find(|(t, v, _)| *t == ty && *v == tag).map(|&(_, _, p)| p)
    }
}
