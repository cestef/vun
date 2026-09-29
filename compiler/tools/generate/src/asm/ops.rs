//! The `Op` enum of an architecture's instructions, in the order the table declares them, and
//! each one's name, mnemonic and forms, each in a module of its own.

use quote::quote;

use super::Instruction;
use super::emit::Crate;
use crate::generate::{Access, Module};
use crate::names::{constant, pascal, raw};

/// The modules of the `Op` enum, its names and mnemonics, and its forms.
const OPS: &str = "ops";
const MNEMONICS: &str = "mnemonics";
const FORMS: &str = "forms";

impl Crate {
    /// The `Op` enum of each instruction, and its mnemonics and forms, each in a module.
    pub(super) fn ops(instructions: &[Instruction<'_>]) -> [Module; 3] {
        let names: Vec<_> = instructions.iter().map(|i| pascal(i.name)).collect();
        let docs = instructions.iter().map(|i| &i.doc);
        let mnemonics = instructions.iter().map(|i| i.mnemonic.as_deref().unwrap_or(i.name));
        let consts = instructions.iter().map(|i| {
            let (module, name) = (raw(i.name), constant(i.name));
            quote! { super::#module::#name }
        });
        let ops = quote! {
            /// An instruction, by its declaration in the architecture's table.
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_variants::Variants)]
            pub enum Op { #(#(#[doc = #docs])* #names,)* }
        };
        let count = instructions.len();
        let declared = instructions.iter().map(|i| i.name);
        let mnemonic = quote! {
            use super::Op;
            /// The name of each instruction as the table declares it, and its mnemonic, in the
            /// order of `Op`.
            const NAMES: [&str; #count] = [#(#declared),*];
            const MNEMONICS: [&str; #count] = [#(#mnemonics),*];
            impl Op {
                /// Its name, as the table declares it (`bcond`).
                #[must_use]
                pub const fn name(self) -> &'static str {
                    NAMES[self as usize]
                }

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
            Module { name: OPS.to_owned(), access: Access::Glob, items: ops, dir: None },
            Module { name: MNEMONICS.to_owned(), access: Access::Impl, items: mnemonic, dir: None },
            Module { name: FORMS.to_owned(), access: Access::Impl, items: forms, dir: None },
        ]
    }
}
