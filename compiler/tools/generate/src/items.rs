//! The declarations of the toolchain's packages, `lang`, `std` and the root of `targets`, as typed Rust handles in
//! `vun-items`, so that Rust code names a declaration by a constant the generator checked exists,
//! never by a path in a string.
//!
//! Each namespace becomes a module of constants, one per public declaration, spelled as the
//! declaration, raw where it is a Rust keyword: `lang.int.unsigned` is `lang::int::unsigned`,
//! `lang.type` is `lang::r#type`. Vun names are unique in a namespace, so handles are too. A
//! package's root namespace is its module itself. The public members of a `struct` literal and
//! the variants of an `enum` literal have handles too, and so do those of the literal a type
//! constructor makes (`lang.Step.yielded`) and of an interface's requirements
//! (`lang.ops.Try.branch`), in a module of their type's path as a nested namespace's is:
//! `std.os.libc.start` is `std::os_libc::start`, `std.build.Build.new` is `std::build_Build::new`.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;
use vun_build::layout::SEPARATOR;
use vun_syntax::literal::Decl;

use crate::generate::{Access, Generator, Module, Read, Source};
use crate::names::{docs, raw};

pub(crate) const LANG: Generator =
    Generator { source: "lib/lang", output: "compiler/base/items/src/lang", emit };
pub(crate) const STD: Generator =
    Generator { source: "lib/std", output: "compiler/base/items/src/std", emit };
/// `targets` has handles for its root file only, such as `targets.hosts`: a target is a file of
/// its own, so adding one generates nothing.
pub(crate) const TARGETS: Generator =
    Generator { source: "lib/targets", output: "compiler/base/items/src/targets", emit: emit_root };

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
    Ok(Handles::of(sources).modules())
}

#[expect(clippy::unnecessary_wraps, reason = "every generator emits through one signature")]
fn emit_root(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let roots = sources.iter().filter(|s| s.namespace().is_some_and(|n| n.len() == 1));
    Ok(Handles::of(roots).modules())
}

/// The handles of each module, by the path it is of: a namespace's declarations and the
/// namespaces inside it, a type's members.
#[derive(Default)]
pub(crate) struct Handles(BTreeMap<Vec<String>, Vec<TokenStream>>);

impl Handles {
    fn of<'a, 's: 'a>(sources: impl IntoIterator<Item = &'a Source<'s>>) -> Self {
        let mut handles = Self::default();
        for source in sources {
            let Some(namespace) = source.namespace() else { continue };
            handles.0.entry(namespace.clone()).or_default();
            for decl in source.decls.iter().filter(|d| !d.name.starts_with(PRIVATE)) {
                handles.declare(&namespace, decl);
            }
            if let [parent @ .., name] = namespace.as_slice()
                && !parent.is_empty()
            {
                let doc = docs(&[&format!("The namespace `{}`.", namespace.join(SEPARATOR))]);
                handles.push(parent, name, &doc);
            }
        }
        handles
    }

    /// The handle of `decl`, declared in `namespace`, and those of its members.
    fn declare(&mut self, namespace: &[String], decl: &Decl<'_>) {
        self.push(namespace, decl.name, &docs(&decl.doc));
        if let Some(members) = decl.value.members() {
            let path: Vec<String> = namespace.iter().cloned().chain([decl.name.to_owned()]).collect();
            for member in members.iter().filter(|m| !m.name.starts_with(PRIVATE)) {
                self.declare(&path, member);
            }
        }
    }

    /// `pub const name: Item`, the handle of the declaration `name` of `namespace`.
    fn push(&mut self, namespace: &[String], name: &str, doc: &[String]) {
        let ident = raw(name);
        let segments = namespace.iter().map(String::as_str).chain([name]);
        let handle = quote! { #(#[doc = #doc])* pub const #ident: Item = Item::new(&[#(#segments),*]); };
        self.0.entry(namespace.to_vec()).or_default().push(handle);
    }

    /// A module per path holding handles: a package's root namespace is the package's module.
    fn modules(self) -> Vec<Module> {
        let modules = self.0.into_iter().filter(|(_, items)| !items.is_empty());
        modules
            .map(|(namespace, items)| {
                let (name, access) = match namespace.as_slice() {
                    [_] => (ROOT.to_owned(), Access::Glob),
                    [_, inner @ ..] => (inner.join(JOIN), Access::Path),
                    [] => unreachable!("a namespace has a package"),
                };
                let doc = format!(" The declarations of `{}`.", namespace.join(SEPARATOR));
                let items = quote! {
                    #![doc = #doc]
                    #![allow(
                        non_upper_case_globals,
                        non_snake_case,
                        reason = "handles and the modules of types' members are spelled as their declarations"
                    )]
                    use crate::Item;
                    #(#items)*
                };
                Module { name, access, items }
            })
            .collect()
    }

    /// The Rust path of the handle of the declaration at the dotted `path`, for generated code of
    /// other crates: `lang.mem.Ptr` is `vun_items::lang::mem::Ptr`, `lang.Option` is
    /// `vun_items::lang::Option`, a package `lang` is `vun_items::lang`.
    pub(crate) fn rust_path(path: &str) -> TokenStream {
        let segments: Vec<&str> = path.split(SEPARATOR).collect();
        let krate = raw(CRATE);
        match segments.as_slice() {
            [package] => {
                let package = raw(package);
                quote! { #krate::#package }
            }
            [package, namespaces @ .., name] => {
                let (package, name) = (raw(package), raw(name));
                if namespaces.is_empty() {
                    quote! { #krate::#package::#name }
                } else {
                    let inner = raw(&namespaces.join(JOIN));
                    quote! { #krate::#package::#inner::#name }
                }
            }
            [] => unreachable!("a path has a segment"),
        }
    }
}
