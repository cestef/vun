//! The kinds and tables of `vun-grammar`, from the files of `lib/lang/lang/syntax`: the grammar
//! and the desugaring tables, read as one set of declarations.
//!
//! Declarations are translated by the shape of their value, never by their name (the rules are in
//! the header of `grammar.vun`), so a new kind, enum or table needs no change here. After changing
//! the token set, generate twice: the parser that reads the grammar is built from the previous
//! output.

use vun_diag::{DiagId, Msg};
use vun_syntax::literal::{Decl, Literal};

use crate::generate::{Generator, Module, Read, Source};

mod enums;
mod slots;
mod tables;

use crate::names::{docs, is_type_name};

pub(crate) const GENERATOR: Generator =
    Generator { source: "lib/lang/lang/syntax", output: "compiler/front/grammar/src/generated", emit };

const SHAPE: DiagId = DiagId::new("xtask.grammar.shape");
const UNRESOLVED: DiagId = DiagId::new("xtask.grammar.unresolved");

/// Fields of a kind's entry written as a record.
const ENTRY_NAME: &str = "name";
const ENTRY_TEXT: &str = "text";

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let decls = sources.iter().flat_map(|s| s.decls.iter().cloned()).collect();
    let source = Source { path: GENERATOR.source.to_owned(), decls };
    Grammar::read(&source)?.emit(&source)
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
    /// The name in `grammar.vun`, by which tables refer to it.
    name: &'s str,
    doc: Vec<String>,
    text: Option<&'s str>,
    /// For a kind entry written `.name(slots...)`, its children.
    slots: Vec<Slot<'s>>,
}

/// One child position of a kind: `.count(.field)`.
struct Slot<'s> {
    count: &'s str,
    field: &'s str,
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
                        .map(|v| Variant { name: v.name, doc: docs(&v.doc), text: None, slots: Vec::new() })
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
                GENERATOR.error(SHAPE, d.name, msg)
            })?;
            let mut names = Vec::new();
            for entry in entries {
                let variant = Variant::entry(entry).ok_or_else(|| {
                    let record = format!(".{{ .{ENTRY_NAME} = .x, .{ENTRY_TEXT} = \"...\" }}");
                    let msg = Msg::new().text("entries are ").code(".name").text(", ").code("\"text\"");
                    let msg = msg.text(", ").code(record).text(" or ").code(".name(.count(.field), ...)");
                    GENERATOR.error(SHAPE, d.name, msg)
                })?;
                names.push(variant.name);
                kind.variants.push(variant);
            }
            kind.lists.push((list.name, names));
        }
        Ok(kind)
    }
}
