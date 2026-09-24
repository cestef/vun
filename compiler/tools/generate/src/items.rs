//! The declarations of the toolchain's packages, `lang` and `std`, as typed Rust handles in
//! `vun-items`, so that Rust code names a declaration by a constant the generator checked exists,
//! never by a path in a string.
//!
//! Each namespace becomes a module of constants, one per public declaration, spelled as the
//! declaration, raw where it is a Rust keyword: `lang.int.unsigned` is `lang::int::unsigned`,
//! `lang.type` is `lang::r#type`. Vun names are unique in a namespace, so handles are too. A
//! package's root namespace is its module itself.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;
use vun_build::layout::SEPARATOR;

use crate::generate::{Access, Generator, Module, Read, Source};
use crate::names::{docs, module};

pub(crate) const LANG: Generator =
    Generator { source: "lib/lang", output: "compiler/base/items/src/lang", emit };
pub(crate) const STD: Generator =
    Generator { source: "lib/std", output: "compiler/base/items/src/std", emit };

/// The crate the handles are generated into.
const CRATE: &str = "vun_items";
/// Joins the segments of a nested namespace in a module name.
const JOIN: &str = "_";
/// The module holding a package's root namespace, whose items the package's module re-exports.
const ROOT: &str = "root";
/// Marks a private declaration, which gets no handle.
const PRIVATE: char = '_';

#[expect(clippy::unnecessary_wraps, reason = "every generator emits through one signature")]
fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    // The handles of each namespace's module: its declarations, and the namespaces inside it.
    let mut modules: BTreeMap<Vec<String>, Vec<TokenStream>> = BTreeMap::new();
    for source in sources {
        let Some(namespace) = source.namespace() else { continue };
        let items = modules.entry(namespace.clone()).or_default();
        for decl in source.decls.iter().filter(|d| !d.name.starts_with(PRIVATE)) {
            items.push(handle(&namespace, decl.name, &docs(&decl.doc)));
        }
        if let [parent @ .., name] = namespace.as_slice()
            && !parent.is_empty()
        {
            let doc = docs(&[&format!("The namespace `{}`.", namespace.join(SEPARATOR))]);
            modules.entry(parent.to_vec()).or_default().push(handle(parent, name, &doc));
        }
    }
    let modules = modules.into_iter().filter(|(_, items)| !items.is_empty()).map(|(namespace, items)| {
        let (name, access) = match namespace.as_slice() {
            [_] => (ROOT.to_owned(), Access::Glob),
            [_, inner @ ..] => (inner.join(JOIN), Access::Path),
            [] => unreachable!("a namespace has a package"),
        };
        let doc = format!(" The declarations of `{}`.", namespace.join(SEPARATOR));
        let items = quote! {
            #![doc = #doc]
            #![allow(non_upper_case_globals, reason = "handles are spelled as their declarations")]
            use crate::Item;
            #(#items)*
        };
        Module { name, access, items }
    });
    Ok(modules.collect())
}

/// The Rust path of the handle of the declaration at the dotted `path`, for generated code of
/// other crates: `lang.mem.Ptr` is `vun_items::lang::mem::Ptr`, `lang.Option` is
/// `vun_items::lang::Option`, a package `lang` is `vun_items::lang`.
pub(crate) fn rust_path(path: &str) -> TokenStream {
    let segments: Vec<&str> = path.split(SEPARATOR).collect();
    let krate = module(CRATE);
    match segments.as_slice() {
        [package] => {
            let package = module(package);
            quote! { #krate::#package }
        }
        [package, namespaces @ .., name] => {
            let (package, name) = (module(package), module(name));
            if namespaces.is_empty() {
                quote! { #krate::#package::#name }
            } else {
                let inner = module(&namespaces.join(JOIN));
                quote! { #krate::#package::#inner::#name }
            }
        }
        [] => unreachable!("a path has a segment"),
    }
}

/// `pub const name: Item`, the handle of the declaration `name` of `namespace`.
fn handle(namespace: &[String], name: &str, doc: &[String]) -> TokenStream {
    let ident = module(name);
    let segments = namespace.iter().map(String::as_str).chain([name]);
    quote! { #(#[doc = #doc])* pub const #ident: Item = Item::new(&[#(#segments),*]); }
}
