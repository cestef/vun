//! The kinds and tables of `overt-grammar`, from `lib/lang/lang/syntax/grammar.ov`.
//!
//! Declarations are translated by the shape of their value, never by their name (the rules are in
//! the header of `grammar.ov`), so a new kind, enum or table needs no change here. After changing
//! the token set, generate twice: the parser that reads the grammar is built from the previous
//! output.

use overt_diag::{DiagId, Diagnostic, Msg};
use overt_syntax::literal::{Decl, Literal};
use proc_macro2::TokenStream;

use crate::generate::{self, Generator, Read, Source};

mod enums;
mod names;
mod tables;

use enums::entry_variant;
use names::{docs, is_type_name};

pub(crate) const GENERATOR: Generator = Generator {
    source: "lib/lang/lang/syntax/grammar.ov",
    output: "compiler/front/grammar/src/generated.rs",
    emit,
};

const SHAPE: DiagId = DiagId::new("xtask.grammar.shape");
const UNRESOLVED: DiagId = DiagId::new("xtask.grammar.unresolved");

/// Fields of a kind's entry written as a record.
const ENTRY_NAME: &str = "name";
const ENTRY_TEXT: &str = "text";

fn emit(source: &Source<'_>) -> Read<TokenStream> {
    Grammar::read(source)?.emit(source)
}

fn error_at(id: DiagId, decl: &str, msg: Msg) -> Box<Diagnostic> {
    generate::error(id, GENERATOR.source, decl, msg)
}

/// An enum to generate: a kind, whose variants come from its lists, or a plain enum.
struct Enum<'s> {
    name: &'s str,
    doc: Vec<String>,
    variants: Vec<Variant<'s>>,
    /// For kinds, each list with the variants it holds, generated as constants.
    lists: Vec<(&'s str, Vec<&'s str>)>,
}

struct Variant<'s> {
    /// The name in `grammar.ov`, by which tables refer to it.
    name: &'s str,
    doc: Vec<String>,
    text: Option<&'s str>,
}

struct Grammar<'s> {
    enums: Vec<Enum<'s>>,
}

/// The Rust type of a table's values, inferred from the values.
enum Ty {
    Str,
    /// An enum, by its index in [`Grammar::enums`].
    Enum(usize),
    List(Box<Ty>),
    /// A record's fields, in the order of the first record.
    Tuple(Vec<(String, Ty)>),
}

impl<'s> Grammar<'s> {
    fn read(source: &Source<'s>) -> Read<Self> {
        let mut enums = Vec::new();
        for d in &source.decls {
            match &d.value {
                Literal::Record(lists) if is_type_name(d.name) => enums.push(Self::kind(d, lists)?),
                Literal::Enum(variants) => enums.push(Enum {
                    name: d.name,
                    doc: docs(&d.doc),
                    variants: variants
                        .iter()
                        .map(|v| Variant { name: v.name, doc: docs(&v.doc), text: None })
                        .collect(),
                    lists: Vec::new(),
                }),
                _ => {}
            }
        }
        Ok(Self { enums })
    }

    fn kind(d: &Decl<'s>, lists: &[Decl<'s>]) -> Read<Enum<'s>> {
        let mut kind = Enum { name: d.name, doc: docs(&d.doc), variants: Vec::new(), lists: Vec::new() };
        for list in lists {
            let entries = list.value.as_list().ok_or_else(|| {
                let msg =
                    Msg::new().text("each field of a kind is a list, but ").code(list.name).text(" is not");
                error_at(SHAPE, d.name, msg)
            })?;
            let mut names = Vec::new();
            for entry in entries {
                let variant = entry_variant(entry).ok_or_else(|| {
                    let record = format!(".{{ .{ENTRY_NAME} = .x, .{ENTRY_TEXT} = \"...\" }}");
                    let msg = Msg::new().text("entries are ").code(".name").text(", ").code("\"text\"");
                    error_at(SHAPE, d.name, msg.text(" or ").code(record))
                })?;
                names.push(variant.name);
                kind.variants.push(variant);
            }
            kind.lists.push((list.name, names));
        }
        Ok(kind)
    }
}
