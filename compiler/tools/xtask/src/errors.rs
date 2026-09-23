//! The diagnostic ids of `overt-diag`, from `lib/lang/lang/errors.ov`.
//!
//! Every declaration of the file is a diagnostic, grouped in namespaces; its id is its binding
//! path, derived from where the file sits in its package (spec 01, section 8). Each namespace also
//! becomes an id, the group that stages declare they report (`DiagId::within`).

use std::path::Path;

use overt_build::layout::{Location, SEPARATOR};
use overt_syntax::literal::{Decl, Literal};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use overt_diag::{DiagId, Msg};

use crate::generate::{self, Generator, Read, Source};

pub(crate) const GENERATOR: Generator =
    Generator { source: "lib/lang/lang/errors.ov", output: "compiler/base/diag/src/errors.rs", emit };

const EMPTY: DiagId = DiagId::new("xtask.errors.empty");

/// Where packages live, relative to the repository root.
const PACKAGES: &str = "lib";

fn emit(source: &Source<'_>) -> Read<TokenStream> {
    let namespace = namespace(source.path);
    let mut ids = Vec::new();
    collect(&mut Vec::new(), &source.decls, &mut ids);
    if ids.is_empty() {
        return Err(generate::error(
            EMPTY,
            source.path,
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
    let names = ids.iter().filter(|id| id.entry == Entry::Diagnostic).map(Id::constant);
    let all_doc = format!(" Every diagnostic declared in `{namespace}`.");
    Ok(quote! {
        use crate::DiagId;
        #(#consts)*
        #[doc = #all_doc]
        pub const ALL: &[DiagId] = &[#(#names),*];
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
}

impl Id<'_> {
    /// `syntax.unknown_character` is `SYNTAX_UNKNOWN_CHARACTER`.
    fn constant(&self) -> Ident {
        Ident::new(&self.within.join("_").to_ascii_uppercase(), Span::call_site())
    }
}

fn collect<'s>(within: &mut Vec<&'s str>, decls: &[Decl<'s>], ids: &mut Vec<Id<'s>>) {
    for d in decls {
        within.push(d.name);
        let doc = d.doc.iter().map(|l| format!(" {l}")).collect();
        if let Literal::Namespace(members) = &d.value {
            ids.push(Id { entry: Entry::Group, within: within.clone(), doc });
            collect(within, members, ids);
        } else {
            ids.push(Id { entry: Entry::Diagnostic, within: within.clone(), doc });
        }
        within.pop();
    }
}

/// The binding path of a source file of a package in [`PACKAGES`]: `lib/lang/lang/errors.ov` is
/// `lang.errors`.
fn namespace(source: &str) -> String {
    let path = Path::new(source);
    let package = path.strip_prefix(PACKAGES).ok().and_then(|p| p.iter().next());
    let names = package.and_then(|name| Location::path_of(&Path::new(PACKAGES).join(name), path));
    names.unwrap_or_default().join(SEPARATOR)
}
