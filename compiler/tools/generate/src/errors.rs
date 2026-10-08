//! The diagnostic ids of `vun-diag`, from the files of `lib/lang/lang/errors/`.
//!
//! Each file is a group, its `//!` lines the group's doc, and each declaration of it a diagnostic;
//! its id is its binding path, derived from where the file sits in its package (spec 01, section
//! 8). Each group also becomes an id, the one that stages declare they report
//! (`DiagId::within`). A diagnostic's
//! explanation page is the file its binding path names on disk, with the extension `DiagId::PAGE`
//! (spec 10, section 1); `EXPLANATIONS` gives each page's path in the library.

use std::path::{Path, PathBuf};

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use vun_build::layout::{Location, SEPARATOR};
use vun_syntax::literal::{Decl, Literal};

use vun_diag::{DiagId, Msg};

use crate::generate::{Access, Emit, Generator, Module, Read, Source};
use crate::names::{constant, docs, pascal};

pub(crate) const GENERATOR: Generator = Generator {
    source: "lib/lang/lang/errors",
    output: "compiler/base/diag/src/errors",
    emit: Emit::Rust(emit),
    keeps: &[],
};

/// The module listing every diagnostic.
const ALL_MODULE: &str = "all";
/// The module giving each diagnostic's explanation page.
const PAGES_MODULE: &str = "pages";
/// The module declaring the enum of every id.
const IDS_MODULE: &str = "ids";
/// The module giving each id's binding path.
const PATHS_MODULE: &str = "paths";

const EMPTY: DiagId = DiagId::new("generate.errors.empty");

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let root = Source::namespace_of(Path::new(GENERATOR.source)).unwrap_or_default();
    let namespace = root.join(SEPARATOR);
    let mut ids = Vec::new();
    for source in sources {
        let group: Vec<String> = source.namespace().unwrap_or_default().split_off(root.len());
        ids.push(Id { entry: Entry::Group, within: group.clone(), doc: docs(&source.doc) });
        Id::collect(&mut group.clone(), &source.decls, &mut ids);
    }
    if ids.iter().all(|id| id.entry == Entry::Group) {
        return Err(GENERATOR.error(EMPTY, &namespace, Msg::new().text("declares no diagnostics")));
    }
    let consts = |group: &str| -> Vec<TokenStream> {
        let ids = ids.iter().filter(|id| id.within.first().map(String::as_str) == Some(group));
        ids.map(|id| {
            let (name, doc, variant) = (id.constant(), &id.doc, id.variant());
            quote! { #(#[doc = #doc])* pub const #name: DiagId = DiagId::Lang(Lang::#variant); }
        })
        .collect()
    };
    let mut groups: Vec<&str> = ids.iter().filter_map(|id| id.within.first().map(String::as_str)).collect();
    groups.dedup();
    let mut modules: Vec<Module> = groups
        .iter()
        .map(|&group| {
            let consts = consts(group);
            Module {
                name: group.to_owned(),
                access: Access::Glob,
                items: quote! { use crate::DiagId; use super::Lang; #(#consts)* },
                dir: None,
            }
        })
        .collect();
    let diagnostics: Vec<&Id> = ids.iter().filter(|id| id.entry == Entry::Diagnostic).collect();
    let names = diagnostics.iter().map(|id| id.constant());
    let pages = diagnostics.iter().map(|id| {
        let (name, path) = (id.constant(), Id::page(&root, &id.within).display().to_string());
        quote! { (#name, #path) }
    });
    let variants = ids.iter().map(|id| {
        let (variant, doc) = (id.variant(), &id.doc);
        quote! { #(#[doc = #doc])* #variant }
    });
    let paths = ids.iter().map(|id| id.path(&namespace));
    let count = ids.len();
    let lang_doc =
        format!(" Every id declared in `{namespace}`, the diagnostics and the groups holding them.");
    let lang = quote! {
        use vun_codec::{Decode, Encode};
        use vun_variants::Variants;
        #[doc = #lang_doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Variants, Encode, Decode)]
        pub enum Lang { #(#variants),* }
    };
    let table = quote! {
        use super::Lang;
        /// The binding path of each id, in the order of [`Lang`].
        const PATHS: [&str; #count] = [#(#paths),*];
        impl Lang {
            /// The binding path of the id.
            #[must_use]
            pub const fn path(self) -> &'static str {
                PATHS[self as usize]
            }
        }
    };
    let all_doc = format!(" Every diagnostic declared in `{namespace}`.");
    let declared_in = GENERATOR.source;
    let all = quote! {
        use crate::DiagId;
        /// The directory declaring every diagnostic, relative to the repository root.
        pub const DECLARED_IN: &str = #declared_in;
        #[doc = #all_doc]
        pub const ALL: &[DiagId] = &[#(#names),*];
    };
    let pages = quote! {
        use crate::DiagId;
        /// The explanation page of each diagnostic, relative to the library.
        pub const EXPLANATIONS: &[(DiagId, &str)] = &[#(#pages),*];
    };
    modules.push(Module::sibling(IDS_MODULE.to_owned(), &lang));
    modules.push(Module { name: PATHS_MODULE.to_owned(), access: Access::Impl, items: table, dir: None });
    modules.push(Module::sibling(ALL_MODULE.to_owned(), &all));
    modules.push(Module::sibling(PAGES_MODULE.to_owned(), &pages));
    Ok(modules)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entry {
    Group,
    Diagnostic,
}

/// A diagnostic or a group: its path below the root namespace, and its doc lines.
pub(crate) struct Id {
    entry: Entry,
    within: Vec<String>,
    doc: Vec<String>,
}

impl Id {
    /// `syntax.unknown_character` is `SYNTAX_UNKNOWN_CHARACTER`.
    fn constant(&self) -> Ident {
        constant(&self.within.join("_"))
    }

    /// `syntax.unknown_character` is `SyntaxUnknownCharacter`.
    fn variant(&self) -> Ident {
        pascal(&self.within.join("_"))
    }

    /// The binding path: the namespace of the file, then the names within it.
    fn path(&self, namespace: &str) -> String {
        let path: Vec<&str> = [namespace].into_iter().chain(self.within.iter().map(String::as_str)).collect();
        path.join(&DiagId::SEPARATOR.to_string())
    }

    /// The explanation page of the diagnostic `within` the namespace `root`, relative to the
    /// directory of packages: the file its binding path names, as for any package's lints.
    pub(crate) fn page(root: &[String], within: &[impl AsRef<str>]) -> PathBuf {
        let path: Vec<&str> =
            root.iter().map(String::as_str).chain(within.iter().map(AsRef::as_ref)).collect();
        let package = path.first().copied().unwrap_or_default();
        Location::at(Path::new(package), &path).with_extension(DiagId::PAGE)
    }
}

impl Id {
    /// The ids declared by `decls`, in the groups `within`, depth first.
    fn collect(within: &mut Vec<String>, decls: &[Decl<'_>], ids: &mut Vec<Self>) {
        for d in decls {
            within.push(d.name.to_owned());
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
