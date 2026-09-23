//! The kinds and tables of `overt-grammar`, from `lib/lang/lang/syntax/grammar.ov`.
//!
//! Declarations are translated by the shape of their value, never by their name (the rules are in
//! the header of `grammar.ov`), so a new kind, enum or table needs no change here. After changing
//! the token set, generate twice: the parser that reads the grammar is built from the previous
//! output.

use std::borrow::Cow;

use overt_diag::{DiagId, Diagnostic, Msg};
use overt_syntax::literal::{Decl, Literal};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};

use crate::generate::{self, Generator, Read, Source};

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

    fn emit(&self, source: &Source<'s>) -> Read<TokenStream> {
        let enums = self.enums.iter().map(Enum::emit);
        let tables = source
            .decls
            .iter()
            .filter_map(|d| d.value.as_list().map(|items| self.table(d, items)))
            .collect::<Read<Vec<_>>>()?;
        Ok(quote! { #(#enums)* #(#tables)* })
    }

    fn table(&self, d: &Decl<'s>, items: &[Literal<'s>]) -> Read<TokenStream> {
        let ty = self.infer(d.name, &items.iter().collect::<Vec<_>>())?;
        let rust_ty = self.rust_type(&ty);
        let values = items.iter().map(|item| self.value(item, &ty));
        let (name, doc) = (constant(d.name), docs(&d.doc));
        Ok(quote! { #(#[doc = #doc])* pub const #name: &[#rust_ty] = &[#(#values),*]; })
    }

    /// The one type of all `values`.
    fn infer(&self, decl: &str, values: &[&Literal<'s>]) -> Read<Ty> {
        let mismatch =
            || error_at(SHAPE, decl, Msg::new().text("the values of a table must share one shape"));
        match values.first() {
            None => Err(error_at(SHAPE, decl, Msg::new().text("an empty list has no type to infer"))),
            Some(Literal::String(_)) if values.iter().all(|v| v.as_str().is_some()) => Ok(Ty::Str),
            Some(Literal::Name(_)) => {
                let names =
                    values.iter().map(|v| v.as_name()).collect::<Option<Vec<_>>>().ok_or_else(mismatch)?;
                self.resolve(decl, &names).map(Ty::Enum)
            }
            Some(Literal::List(_)) => {
                let lists =
                    values.iter().map(|v| v.as_list()).collect::<Option<Vec<_>>>().ok_or_else(mismatch)?;
                let items: Vec<_> = lists.into_iter().flatten().collect();
                self.infer(decl, &items).map(|t| Ty::List(Box::new(t)))
            }
            Some(Literal::Record(first)) => first
                .iter()
                .map(|field| {
                    let column = values
                        .iter()
                        .map(|v| v.get(field.name))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(mismatch)?;
                    Ok((field.name.to_owned(), self.infer(decl, &column)?))
                })
                .collect::<Read<_>>()
                .map(Ty::Tuple),
            Some(_) => Err(mismatch()),
        }
    }

    /// The one enum that has every name in `names`.
    fn resolve(&self, decl: &str, names: &[&str]) -> Read<usize> {
        let has_all = |e: &Enum<'_>| names.iter().all(|n| e.variants.iter().any(|v| v.name == *n));
        let found: Vec<_> = (0..self.enums.len()).filter(|&i| has_all(&self.enums[i])).collect();
        match found[..] {
            [one] => Ok(one),
            [] => {
                let names = names.iter().map(|n| format!(".{n}"));
                Err(error_at(
                    UNRESOLVED,
                    decl,
                    Msg::new().text("no kind or enum has all of ").code_list(names),
                ))
            }
            _ => {
                let candidates = found.iter().map(|&i| self.enums[i].name);
                Err(error_at(
                    UNRESOLVED,
                    decl,
                    Msg::new().text("the names fit several enums: ").code_list(candidates),
                ))
            }
        }
    }

    fn rust_type(&self, ty: &Ty) -> TokenStream {
        match ty {
            Ty::Str => quote! { &str },
            Ty::Enum(i) => ident(self.enums[*i].name).into_token_stream(),
            Ty::List(item) => {
                let item = self.rust_type(item);
                quote! { &[#item] }
            }
            Ty::Tuple(fields) => {
                let fields = fields.iter().map(|(_, t)| self.rust_type(t));
                quote! { (#(#fields),*) }
            }
        }
    }

    /// A value of type `ty`, whose shape `infer` checked.
    fn value(&self, value: &Literal<'_>, ty: &Ty) -> TokenStream {
        match (ty, value) {
            (Ty::Str, Literal::String(text)) => quote! { #text },
            (Ty::Enum(i), Literal::Name(name)) => {
                let (e, v) = (ident(self.enums[*i].name), pascal(name));
                quote! { #e::#v }
            }
            (Ty::List(item), Literal::List(items)) => {
                let items = items.iter().map(|v| self.value(v, item));
                quote! { &[#(#items),*] }
            }
            (Ty::Tuple(fields), Literal::Record(_)) => {
                let values = fields.iter().filter_map(|(name, t)| Some(self.value(value.get(name)?, t)));
                quote! { (#(#values),*) }
            }
            _ => unreachable!("infer checked every value against its table's type"),
        }
    }
}

impl Enum<'_> {
    fn emit(&self) -> TokenStream {
        let name = ident(self.name);
        let doc = &self.doc;
        let variants = self.variants.iter().map(|v| {
            let (ident, doc) = (pascal(v.name), &v.doc);
            quote! { #(#[doc = #doc])* #ident }
        });
        let lists = self.lists.iter().map(|(list, members)| {
            let (list, members) = (constant(list), members.iter().map(|m| pascal(m)));
            quote! { pub const #list: &[Self] = &[#(Self::#members),*]; }
        });
        let spelled: Vec<_> = self.variants.iter().filter_map(|v| Some((pascal(v.name), v.text?))).collect();
        let text = (!spelled.is_empty()).then(|| {
            let (variants, texts): (Vec<_>, Vec<_>) = spelled.into_iter().unzip();
            quote! {
                /// The fixed spelling of this kind, if it has one.
                #[must_use]
                pub const fn text(self) -> Option<&'static str> {
                    match self {
                        #(Self::#variants => Some(#texts),)*
                        _ => None,
                    }
                }
            }
        });
        let body = (!self.lists.is_empty()).then(|| quote! { impl #name { #(#lists)* #text } });
        quote! {
            #(#[doc = #doc])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum #name { #(#variants),* }
            #body
        }
    }
}

/// A kind's entry: `.name`, a spelling `"text"` named after itself, or `.{ .name, .text }`.
fn entry_variant<'s>(entry: &Literal<'s>) -> Option<Variant<'s>> {
    let (name, text) = match entry {
        Literal::Name(name) => (*name, None),
        Literal::String(Cow::Borrowed(text)) => (*text, Some(*text)),
        Literal::Record(_) => match (entry.get(ENTRY_NAME)?, entry.get(ENTRY_TEXT)?) {
            (Literal::Name(name), Literal::String(Cow::Borrowed(text))) => (*name, Some(*text)),
            _ => return None,
        },
        _ => return None,
    };
    Some(Variant { name, doc: Vec::new(), text })
}

fn is_type_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// Doc lines as `#[doc]` values, which carry the space after `///`.
fn docs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| format!(" {l}")).collect()
}

fn ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

/// The Rust spelling of a `snake_case` grammar name: `colon_eq` is `ColonEq`.
fn pascal(name: &str) -> Ident {
    let word: String = name
        .split('_')
        .flat_map(|part| {
            let mut chars = part.chars();
            chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars)
        })
        .collect();
    ident(&word)
}

/// The Rust constant for a table: `element_types` is `ELEMENT_TYPES`.
fn constant(name: &str) -> Ident {
    ident(&name.to_ascii_uppercase())
}
