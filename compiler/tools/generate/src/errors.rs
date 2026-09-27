//! The diagnostic ids of `vun-diag`, from `lib/lang/lang/errors.vun`.
//!
//! Every declaration of the file is a diagnostic, grouped in namespaces; its id is its binding
//! path, derived from where the file sits in its package (spec 01, section 8). Each namespace also
//! becomes an id, the group that stages declare they report (`DiagId::within`). A diagnostic's
//! explanation page is the file its `.explanation = lang.meta.embed("...")` names, relative to the
//! source; `EXPLANATIONS` gives each page's path in the library.

use std::path::Path;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use vun_syntax::literal::{Decl, Literal};

use vun_diag::{DiagId, Msg};

use crate::generate::{self, Access, Generator, Module, Read, Source};
use crate::names::{constant, docs};

pub(crate) const GENERATOR: Generator =
    Generator { source: "lib/lang/lang/errors.vun", output: "compiler/base/diag/src/errors", emit };

/// The module listing every diagnostic and its explanation page.
const ALL_MODULE: &str = "all";

const EMPTY: DiagId = DiagId::new("generate.errors.empty");
const UNEXPLAINED: DiagId = DiagId::new("generate.errors.unexplained");

/// The field of `lang.diag.Diag` naming the explanation, and the call that embeds a file.
const EXPLANATION: &str = "explanation";
const EMBED: [&str; 3] = ["lang", "meta", "embed"];

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    // The source is one file, which `Generator::generate` found.
    let source = &sources[0];
    let namespace = source.namespace_text();
    let mut ids = Vec::new();
    Id::collect(&mut Vec::new(), &source.decls, &mut ids);
    if ids.is_empty() {
        return Err(GENERATOR.error(EMPTY, &namespace, Msg::new().text("declares no diagnostics")));
    }
    let consts = |group: &str| -> Vec<TokenStream> {
        let ids = ids.iter().filter(|id| id.within.first() == Some(&group));
        ids.map(|id| {
            let (name, doc) = (id.constant(), &id.doc);
            let path = [namespace.as_str()].into_iter().chain(id.within.iter().copied()).collect::<Vec<_>>();
            let path = path.join(&DiagId::SEPARATOR.to_string());
            quote! { #(#[doc = #doc])* pub const #name: DiagId = DiagId::new(#path); }
        })
        .collect()
    };
    let mut groups: Vec<&str> = ids.iter().filter_map(|id| id.within.first().copied()).collect();
    groups.dedup();
    let mut modules: Vec<Module> = groups
        .iter()
        .map(|&group| {
            let consts = consts(group);
            Module {
                name: group.to_owned(),
                access: Access::Glob,
                items: quote! { use crate::DiagId; #(#consts)* },
            }
        })
        .collect();
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
            return Err(GENERATOR.error(UNEXPLAINED, &name, msg));
        };
        let (name, path) = (id.constant(), dir.join(page).display().to_string());
        pages.push(quote! { (#name, #path) });
    }
    let all_doc = format!(" Every diagnostic declared in `{namespace}`.");
    let declared_in = GENERATOR.source;
    let all = quote! {
        use crate::DiagId;
        /// The file declaring every diagnostic, relative to the repository root.
        pub const DECLARED_IN: &str = #declared_in;
        #[doc = #all_doc]
        pub const ALL: &[DiagId] = &[#(#names),*];
        /// The explanation page of each diagnostic, relative to the library.
        pub const EXPLANATIONS: &[(DiagId, &str)] = &[#(#pages),*];
    };
    modules.push(Module::sibling(ALL_MODULE.to_owned(), &all));
    Ok(modules)
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
        constant(&self.within.join("_"))
    }
}

impl<'a, 's: 'a> Id<'a> {
    /// The ids declared by `decls`, in the groups `within`, depth first.
    fn collect(within: &mut Vec<&'s str>, decls: &'a [Decl<'s>], ids: &mut Vec<Self>) {
        for d in decls {
            within.push(d.name);
            let doc = docs(&d.doc);
            if let Literal::Namespace(members) = &d.value {
                ids.push(Self { entry: Entry::Group, within: within.clone(), doc, page: None });
                Self::collect(within, members, ids);
            } else {
                let embed =
                    d.value.get(EXPLANATION).and_then(Literal::as_call).filter(|(path, _)| *path == EMBED);
                let page = embed.and_then(|(_, args)| args.first()).and_then(Literal::as_str);
                ids.push(Self { entry: Entry::Diagnostic, within: within.clone(), doc, page });
            }
            within.pop();
        }
    }
}
