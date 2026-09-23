//! The diagnostic ids of `overt-diag`, from `lib/lang/lang/errors.ov`.
//!
//! Every declaration of the file is a diagnostic, grouped in namespaces; its id is its binding
//! path, derived from where the file sits in its package (spec 01, section 8). Each namespace also
//! becomes an id, the group that stages declare they report (`DiagId::within`). A diagnostic's
//! explanation page is the file its `.explanation = meta.embed("...")` names, relative to the
//! source; `EXPLANATIONS` gives each page's path in the library.

use std::path::Path;

use overt_syntax::literal::{Decl, Literal};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use overt_diag::{DiagId, Msg};

use crate::generate::{self, Generator, Read, Source};
use crate::names::docs;

pub(crate) const GENERATOR: Generator =
    Generator { source: "lib/lang/lang/errors.ov", output: "compiler/base/diag/src/errors.rs", emit };

const EMPTY: DiagId = DiagId::new("xtask.errors.empty");
const UNEXPLAINED: DiagId = DiagId::new("xtask.errors.unexplained");

/// The field of `lang.diag.Diag` naming the explanation, and the call that embeds a file.
const EXPLANATION: &str = "explanation";
const EMBED: [&str; 2] = ["meta", "embed"];

fn emit(sources: &[Source<'_>]) -> Read<TokenStream> {
    // The source is one file.
    let source = &sources[0];
    let namespace = source.namespace_text();
    let mut ids = Vec::new();
    collect(&mut Vec::new(), &source.decls, &mut ids);
    if ids.is_empty() {
        return Err(generate::error(
            EMPTY,
            &source.path,
            &namespace,
            Msg::new().text("declares no diagnostics"),
        ));
    }
    let consts = ids.iter().map(|id| {
        let (name, doc) = (id.constant(), &id.doc);
        let path = [namespace.as_str()].into_iter().chain(id.within.iter().copied()).collect::<Vec<_>>();
        let path = path.join(&DiagId::SEPARATOR.to_string());
        quote! { #(#[doc = #doc])* pub const #name: DiagId = DiagId::new(#path); }
    });
    let diagnostics: Vec<&Id<'_>> = ids.iter().filter(|id| id.entry == Entry::Diagnostic).collect();
    let names = diagnostics.iter().map(|id| id.constant());
    let dir = Path::new(&source.path)
        .parent()
        .and_then(|d| d.strip_prefix(generate::PACKAGES).ok())
        .unwrap_or(Path::new(""));
    let mut pages = Vec::new();
    for id in &diagnostics {
        let Some(page) = id.page else {
            let name = id.within.join(&DiagId::SEPARATOR.to_string());
            let msg =
                Msg::new().text("has no ").code(format!(".{EXPLANATION} = {}(\"...\")", EMBED.join(".")));
            return Err(generate::error(UNEXPLAINED, &source.path, &name, msg));
        };
        let (name, path) = (id.constant(), dir.join(page).display().to_string());
        pages.push(quote! { (#name, #path) });
    }
    let all_doc = format!(" Every diagnostic declared in `{namespace}`.");
    Ok(quote! {
        use crate::DiagId;
        #(#consts)*
        #[doc = #all_doc]
        pub const ALL: &[DiagId] = &[#(#names),*];
        /// The explanation page of each diagnostic, relative to the library.
        pub const EXPLANATIONS: &[(DiagId, &str)] = &[#(#pages),*];
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entry {
    Group,
    Diagnostic,
}

/// A diagnostic or a group: its path inside the file, and its doc lines.
struct Id<'s> {
    entry: Entry,
    within: Vec<&'s str>,
    doc: Vec<String>,
    /// The explanation page, relative to the source.
    page: Option<&'s str>,
}

impl Id<'_> {
    /// `syntax.unknown_character` is `SYNTAX_UNKNOWN_CHARACTER`.
    fn constant(&self) -> Ident {
        Ident::new(&self.within.join("_").to_ascii_uppercase(), Span::call_site())
    }
}

fn collect<'a, 's>(within: &mut Vec<&'s str>, decls: &'a [Decl<'s>], ids: &mut Vec<Id<'a>>)
where
    's: 'a,
{
    for d in decls {
        within.push(d.name);
        let doc = docs(&d.doc);
        if let Literal::Namespace(members) = &d.value {
            ids.push(Id { entry: Entry::Group, within: within.clone(), doc, page: None });
            collect(within, members, ids);
        } else {
            let embed =
                d.value.get(EXPLANATION).and_then(Literal::as_call).filter(|(path, _)| *path == EMBED);
            let page = embed.and_then(|(_, args)| args.first()).and_then(Literal::as_str);
            ids.push(Id { entry: Entry::Diagnostic, within: within.clone(), doc, page });
        }
        within.pop();
    }
}
