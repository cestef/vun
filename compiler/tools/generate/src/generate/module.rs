//! The Rust a generator emits: its modules, each a file of the output directory, and the
//! `mod.rs` declaring them, directly or through the directory modules they are declared in.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;
use vun_diag::Msg;

use super::{COLLISION, Generator, Read};
use crate::names::raw;

/// One file of a generated directory.
pub(crate) struct Module {
    /// The module and file name, without extension.
    pub(crate) name: String,
    pub(crate) access: Access,
    /// Its items, with the imports they need.
    pub(crate) items: TokenStream,
    /// The directory module it is declared in, whose items the output's root re-exports, so a
    /// long output keeps each file short; `None` for the root's own.
    pub(crate) dir: Option<String>,
}

/// How the items of a generated module are reached.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    /// As the directory's own: `pub use name::*`.
    Glob,
    /// Through the module's path: `pub mod name`.
    Path,
    /// It only adds methods to the types of its siblings, reached through them: `mod name`.
    Impl,
    /// Its items are its siblings' alone, which name them by path: `mod name`.
    Sibling,
}

impl Module {
    /// A module of one generated directory whose items name those of its siblings.
    pub(crate) fn sibling(name: String, items: &TokenStream) -> Self {
        let items = quote! {
            #[allow(unused_imports, clippy::wildcard_imports, reason = "generated modules share one namespace")]
            use super::*;
            #items
        };
        Self { name, access: Access::Glob, items, dir: None }
    }
}

impl Module {
    /// The module name of the namespace at `path` inside its package, its segments below the
    /// package joined: `lang.mem.raw` is `mem_raw`; `None` for the package's own namespace.
    pub(crate) fn nested(path: &[impl AsRef<str>]) -> Option<String> {
        let inner: Vec<&str> = path.iter().skip(1).map(AsRef::as_ref).collect();
        (!inner.is_empty()).then(|| inner.join(JOIN))
    }
}

/// Joins the namespaces below a package in one module name.
pub(crate) const JOIN: &str = "_";

/// The name of a directory module's root file.
const ROOT: &str = "mod";

impl Generator {
    /// The formatted files of `modules`, each opening with `header`, by path in the output
    /// directory.
    pub(super) fn files(&self, modules: Vec<Module>, header: &str) -> Read<BTreeMap<String, String>> {
        let format = |items: TokenStream| {
            let file = syn::parse2(quote! { #![doc = #header] #items }).expect("generators emit Rust items");
            prettyplease::unparse(&file)
        };
        let declared = |m: &Module| {
            let name = raw(&m.name);
            match m.access {
                Access::Glob => quote! { mod #name; pub use #name::*; },
                Access::Path => quote! { pub mod #name; },
                Access::Impl | Access::Sibling => quote! { mod #name; },
            }
        };
        // Each directory module declares its modules, and the root re-exports its items.
        let mut dirs: BTreeMap<&str, Vec<TokenStream>> = BTreeMap::new();
        let mut root = Vec::new();
        for module in &modules {
            match &module.dir {
                Some(dir) => dirs.entry(dir).or_default().push(declared(module)),
                None => root.push(declared(module)),
            }
        }
        root.extend(dirs.keys().map(|dir| {
            let dir = raw(dir);
            quote! { mod #dir; pub use #dir::*; }
        }));
        let mut files = BTreeMap::from([(format!("{ROOT}.rs"), format(quote! { #(#root)* }))]);
        for (dir, declared) in dirs {
            files.insert(format!("{dir}/{ROOT}.rs"), format(quote! { #(#declared)* }));
        }
        for module in modules {
            let name = match &module.dir {
                Some(dir) => format!("{dir}/{}.rs", module.name),
                None => format!("{}.rs", module.name),
            };
            if files.contains_key(&name) {
                let msg = Msg::new().text("generates the module ").code(&module.name).text(" twice");
                return Err(self.error(COLLISION, &module.name, msg));
            }
            files.insert(name, format(module.items));
        }
        Ok(files)
    }
}
