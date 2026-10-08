//! The kinds and tables of `lib/lang/lang/syntax`, read as one set of declarations so that a
//! table may name any kind, and written apart: `grammar.vun` and `nodes.vun` into `vun-grammar`, which the lexer
//! and parser (and so this generator) are built from, `desugar.vun` into `vun-desugar`, which
//! only the compiler's later stages read. A desugaring table can then never keep the generator
//! from building (decision 132).
//!
//! Declarations are translated into Rust by the shape of their value, never by their name (the
//! rules are in the header of `grammar.vun`), so a new kind, enum or table needs no change here.
//! What the generator interprets, productions and the tables editors and the parser share, it reads
//! by type into one resolved [`model::Model`], which every emitter writes from. After changing the
//! token set, generate twice: the parser that reads the grammar is built from the previous output.

use std::borrow::Cow;

use std::collections::HashSet;
use std::path::Path;

use proc_macro2::TokenStream;
use quote::quote;
use vun_diag::{DiagId, Msg};
use vun_syntax::literal::{Decl, Fields, FromLiteral, Literal};

use crate::generate::{Emit, Generator, Module, Read, Source};

mod editors;
mod enums;
mod lexical;
mod model;
mod pattern;
mod slots;
mod sublime;
mod tables;
mod tree_sitter;

use crate::names::docs;
use model::{Entry, Model};

pub(crate) use editors::{HELIX, NVIM, VSCODE, ZED};
pub(crate) use sublime::SUBLIME;
pub(crate) use tree_sitter::TREE_SITTER;

/// The directory both files are in.
const SYNTAX: &str = "lib/lang/lang/syntax";
/// The attribute marking a record of lists as a kind.
pub(crate) const KIND: &[&str] = &["syntax", "kind"];

pub(crate) const GENERATOR: Generator = Generator {
    source: SYNTAX,
    output: "compiler/lang/grammar/src/generated",
    emit: Emit::Rust(emit_grammar),
    keeps: &[],
};
pub(crate) const DESUGAR: Generator = Generator {
    source: SYNTAX,
    output: "compiler/front/desugar/src/generated",
    emit: Emit::Rust(emit_desugar),
    keeps: &[],
};

/// The stem of the file that goes to `vun-desugar`; every other goes to `vun-grammar`.
const DESUGAR_FILE: &str = "desugar";

/// The part of the directory a file belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Grammar,
    Desugar,
}

impl Part {
    fn of(source: &Source<'_>) -> Self {
        let desugar = Path::new(&source.path).file_stem().is_some_and(|f| f == DESUGAR_FILE);
        if desugar { Self::Desugar } else { Self::Grammar }
    }
}

const SHAPE: DiagId = DiagId::new("generate.grammar.shape");
const UNRESOLVED: DiagId = DiagId::new("generate.grammar.unresolved");

fn emit_grammar(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    emit(sources, Part::Grammar, &TokenStream::new())
}

fn emit_desugar(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let uses = quote! {
        #[allow(unused_imports, clippy::wildcard_imports, reason = "tables name the grammar's kinds")]
        use vun_grammar::*;
    };
    emit(sources, Part::Desugar, &uses)
}

/// The kinds and tables declared in the files of `part`, resolved against every file's kinds; each
/// module also has `uses`, the imports of kinds declared elsewhere.
fn emit(sources: &[Source<'_>], part: Part, uses: &TokenStream) -> Read<Vec<Module>> {
    let decls = sources.iter().flat_map(|s| s.decls.iter().cloned()).collect();
    let all = Source { path: SYNTAX.to_owned(), doc: Vec::new(), decls };
    let own = sources.iter().filter(|s| Part::of(s) == part);
    let own: HashSet<&str> = own.flat_map(|s| s.decls.iter().map(|d| d.name)).collect();
    let model = Model::read(sources)?;
    Grammar::read(&all, &model)?.emit(&all, &own, uses, &model)
}

/// An enum to generate: a kind, whose variants come from its lists, or a plain enum.
struct Enum<'s> {
    /// Its name; derived enums, as the fields of a kind with productions, are named by the
    /// generator.
    name: Cow<'s, str>,
    doc: Vec<String>,
    variants: Vec<Variant<'s>>,
    /// For kinds, each list with the variants it holds, generated as constants.
    lists: Vec<(String, Vec<String>)>,
}

struct Variant<'s> {
    /// The name in `grammar.vun`, by which tables refer to it.
    name: Cow<'s, str>,
    doc: Vec<String>,
    /// Its fixed spelling.
    text: Option<String>,
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
    /// The kinds and enums of `source`, and the fields of the model's productions as an enum,
    /// which tables may name.
    fn read(source: &Source<'s>, model: &Model) -> Read<Self> {
        let mut enums = Vec::new();
        for d in &source.decls {
            match &d.value {
                Literal::Record(_) if d.marked(KIND) => {
                    let lists = Fields::<Vec<Entry>>::from_literal(&d.value).map_err(|m| {
                        let msg = Msg::new()
                            .text("a kind is a record of lists of entries, which are ")
                            .code(".name");
                        let msg = msg
                            .text(", ")
                            .code("\"text\"")
                            .text(", ")
                            .code(".{ .name = .x, .text = \"...\" }");
                        Box::new(
                            (*GENERATOR.error(
                                SHAPE,
                                d.name,
                                msg.text(" or a production ").code(".name(terms...)"),
                            ))
                            .caused_by(&m),
                        )
                    })?;
                    enums.push(Enum::kind(d, lists));
                }
                Literal::Enum(variants, _) => enums.push(Enum {
                    name: Cow::Borrowed(d.name),
                    doc: docs(&d.doc),
                    variants: variants
                        .iter()
                        .map(|v| Variant { name: Cow::Borrowed(v.name), doc: docs(&v.doc), text: None })
                        .collect(),
                    lists: Vec::new(),
                }),
                _ => {}
            }
        }
        let fields = enums.iter().find(|e| e.is_nodes(model)).map(|nodes| nodes.fields(model));
        enums.extend(fields);
        Ok(Self { enums })
    }
}

impl<'s> Enum<'s> {
    fn kind(d: &Decl<'s>, lists: Fields<Vec<Entry>>) -> Self {
        let mut kind =
            Enum { name: Cow::Borrowed(d.name), doc: docs(&d.doc), variants: Vec::new(), lists: Vec::new() };
        for (list, entries) in lists.0 {
            let names = entries.iter().map(|e| e.name.clone()).collect();
            kind.variants.extend(entries.into_iter().map(|e| Variant {
                name: Cow::Owned(e.name),
                doc: Vec::new(),
                text: e.text,
            }));
            kind.lists.push((list, names));
        }
        kind
    }
}
