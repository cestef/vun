//! The diagnostic ids of `vun-diag`, from `lib/lang/lang/errors.vun`.
//!
//! Every declaration of the file is a diagnostic, grouped in namespaces; its id is its binding
//! path, derived from where the file sits in its package (spec 01, section 8). Each namespace also
//! becomes an id, the group that stages declare they report (`DiagId::within`). A diagnostic's
//! explanation page is the file its binding path names on disk, with the extension `DiagId::PAGE`
//! (spec 10, section 1); `EXPLANATIONS` gives each page's path in the library.

use std::path::{Path, PathBuf};

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use vun_build::layout::Location;
use vun_syntax::literal::{Decl, Literal};

use vun_diag::{DiagId, Msg};

use crate::generate::{Access, Emit, Generator, Module, Read, Source};
use crate::names::{constant, docs};

pub(crate) const GENERATOR: Generator = Generator {
    source: "lib/lang/lang/errors.vun",
    output: "compiler/base/diag/src/errors",
    emit: Emit::Rust(emit),
};

/// The module listing every diagnostic and its explanation page.
const ALL_MODULE: &str = "all";

const EMPTY: DiagId = DiagId::new("generate.errors.empty");

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
    let file = source.namespace().unwrap_or_default();
    let pages = diagnostics.iter().map(|id| {
        let (name, path) = (id.constant(), Id::page(&file, &id.within).display().to_string());
        quote! { (#name, #path) }
    });
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
pub(crate) struct Id<'s> {
    entry: Entry,
    within: Vec<&'s str>,
    doc: Vec<String>,
}

impl Id<'_> {
    /// `syntax.unknown_character` is `SYNTAX_UNKNOWN_CHARACTER`.
    fn constant(&self) -> Ident {
        constant(&self.within.join("_"))
    }

    /// The explanation page of the diagnostic `within` the namespace `file`, relative to the
    /// directory of packages: the file its binding path names, as for any package's lints.
    pub(crate) fn page(file: &[String], within: &[&str]) -> PathBuf {
        let path: Vec<&str> = file.iter().map(String::as_str).chain(within.iter().copied()).collect();
        let package = path.first().copied().unwrap_or_default();
        Location::at(Path::new(package), &path).with_extension(DiagId::PAGE)
    }
}

impl<'a, 's: 'a> Id<'a> {
    /// The ids declared by `decls`, in the groups `within`, depth first.
    fn collect(within: &mut Vec<&'s str>, decls: &'a [Decl<'s>], ids: &mut Vec<Self>) {
        for d in decls {
            within.push(d.name);
            let doc = docs(&d.doc);
            if let Literal::Namespace(members) = &d.value {
                ids.push(Self { entry: Entry::Group, within: within.clone(), doc });
                Self::collect(within, members, ids);
            } else {
                ids.push(Self { entry: Entry::Diagnostic, within: within.clone(), doc });
            }
            within.pop();
        }
    }
}
