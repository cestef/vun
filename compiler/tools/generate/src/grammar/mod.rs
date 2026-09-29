//! The kinds and tables of `lib/lang/lang/syntax`, read as one set of declarations so that a
//! table may name any kind, and written apart: `grammar.vun` into `vun-grammar`, which the lexer
//! and parser (and so this generator) are built from, `desugar.vun` into `vun-desugar`, which
//! only the compiler's later stages read. A desugaring table can then never keep the generator
//! from building (decision 132).
//!
//! Declarations are translated by the shape of their value, never by their name (the rules are in
//! the header of `grammar.vun`), so a new kind, enum or table needs no change here. After changing
//! the token set, generate twice: the parser that reads the grammar is built from the previous
//! output.

use std::borrow::Cow;

use std::collections::HashSet;
use std::path::Path;

use proc_macro2::TokenStream;
use quote::quote;
use vun_diag::{DiagId, Msg};
use vun_syntax::literal::{Decl, Literal};

use crate::generate::{Emit, Generator, Module, Read, Source};

mod editor;
mod enums;
mod slots;
mod tables;

use crate::names::{Name, docs};

pub(crate) use editor::EDITOR;

/// The directory both files are in.
const SYNTAX: &str = "lib/lang/lang/syntax";

pub(crate) const GENERATOR: Generator = Generator {
    source: SYNTAX,
    output: "compiler/lang/grammar/src/generated",
    emit: Emit::Rust(emit_grammar),
};
pub(crate) const DESUGAR: Generator = Generator {
    source: SYNTAX,
    output: "compiler/front/desugar/src/generated",
    emit: Emit::Rust(emit_desugar),
};

/// The file of each, by stem.
const GRAMMAR_FILE: &str = "grammar";
const DESUGAR_FILE: &str = "desugar";

const SHAPE: DiagId = DiagId::new("generate.grammar.shape");
const UNRESOLVED: DiagId = DiagId::new("generate.grammar.unresolved");

/// Fields of a kind's entry written as a record.
const ENTRY_NAME: &str = "name";
const ENTRY_TEXT: &str = "text";

fn emit_grammar(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    emit(sources, GRAMMAR_FILE, &TokenStream::new())
}

fn emit_desugar(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let uses = quote! {
        #[allow(unused_imports, clippy::wildcard_imports, reason = "tables name the grammar's kinds")]
        use vun_grammar::*;
    };
    emit(sources, DESUGAR_FILE, &uses)
}

/// The kinds and tables declared in the file `stem`, resolved against every file's kinds; each
/// module also has `uses`, the imports of kinds declared elsewhere.
fn emit(sources: &[Source<'_>], stem: &str, uses: &TokenStream) -> Read<Vec<Module>> {
    let decls = sources.iter().flat_map(|s| s.decls.iter().cloned()).collect();
    let all = Source { path: SYNTAX.to_owned(), decls };
    let own = sources.iter().filter(|s| Path::new(&s.path).file_stem().is_some_and(|f| f == stem));
    let own: HashSet<&str> = own.flat_map(|s| s.decls.iter().map(|d| d.name)).collect();
    Grammar::read(&all)?.emit(&all, &own, uses)
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
    /// Its fixed spelling, escaped in the source or not.
    text: Option<Cow<'s, str>>,
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
    /// A declaration of `lang`, written as its path: its handle in `vun-items`.
    Item,
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
                Literal::Record(lists) if d.name.is_type_name() => enums.push(Self::kind(d, lists)?),
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
