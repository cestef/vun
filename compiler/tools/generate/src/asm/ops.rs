//! The `Op` enum of an architecture's instructions, in the order the table declares them, and
//! each one's name, mnemonic and forms, each in a module of its own.

use quote::quote;

use super::Instruction;
use super::emit::Crate;
use crate::generate::{Access, Module};
use crate::names::{constant, pascal, raw};

/// The modules of the `Op` enum, its names and mnemonics, and its forms.
const OPS: &str = "ops";
const NAMES: &str = "names";
const MNEMONICS: &str = "mnemonics";
const FORMS: &str = "forms";

/// How many instructions a directory of the forms' modules holds.
pub(super) const CHUNK: usize = 100;

impl Crate {
    /// The `Op` enum of each instruction, and its mnemonics and forms, each in a module. The lists
    /// are written through macros taking their items spaced and not separated, which `prettyplease`
    /// fills lines with, so that a table of hundreds of instructions stays within a file's limit.
    pub(super) fn ops(instructions: &[Instruction<'_>]) -> [Module; 4] {
        let names: Vec<_> = instructions.iter().map(|i| pascal(i.name)).collect();
        let mnemonics = instructions.iter().map(|i| i.mnemonic.as_deref().unwrap_or(i.name));
        let forms: Vec<_> = instructions.iter().map(|i| (raw(i.name), constant(i.name))).collect();
        let (modules, consts): (Vec<_>, Vec<_>) = forms.into_iter().unzip();
        let ops = quote! {
            macro_rules! declare {
                ($($name:ident)*) => {
                    /// An instruction, by its declaration in the architecture's table.
                    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_variants::Variants)]
                    pub enum Op { $($name,)* }
                };
            }
            declare! { #(#names)* }
        };
        let count = instructions.len();
        let declared = instructions.iter().map(|i| i.name);
        let names = quote! {
            use super::Op;
            macro_rules! list {
                ($($item:literal)*) => { [$($item),*] };
            }
            /// The name of each instruction as the table declares it, in the order of `Op`.
            const NAMES: [&str; #count] = list! { #(#declared)* };
            impl Op {
                /// Its name, as the table declares it (`bcond`).
                #[must_use]
                pub const fn name(self) -> &'static str {
                    NAMES[self as usize]
                }
            }
        };
        let mnemonic = quote! {
            use super::Op;
            macro_rules! list {
                ($($item:literal)*) => { [$($item),*] };
            }
            /// The mnemonic of each instruction, in the order of `Op`.
            const MNEMONICS: [&str; #count] = list! { #(#mnemonics)* };
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
            macro_rules! list {
                ($($module:ident $name:ident)*) => { [$(&super::$module::$name),*] };
            }
            /// The forms of each instruction, in the order of `Op`.
            const FORMS: [&[Form]; #count] = list! { #(#modules #consts)* };
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
            Module { name: NAMES.to_owned(), access: Access::Impl, items: names, dir: None },
            Module { name: MNEMONICS.to_owned(), access: Access::Impl, items: mnemonic, dir: None },
            Module { name: FORMS.to_owned(), access: Access::Impl, items: forms, dir: None },
        ]
    }
}
