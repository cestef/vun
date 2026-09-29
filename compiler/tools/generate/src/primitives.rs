//! The primitives of `vun-primitives`, from `lib/lang`: every declaration of the package's namespace
//! whose value is `_` is provided by the compiler (spec 00, the layering rule), so those
//! declarations are the table of what the compiler implements.
//!
//! Only declarations at the top level of a namespace count. A hole inside an interface's
//! requirement struct, such as `add: (a: Self, b: Rhs) -> Output = _;` in `lang.ops.Add`, is a
//! requirement, not a primitive: it sits inside the call to `interface(...)`, which the literal
//! reader leaves opaque.
//!
//! Each namespace becomes a module with one enum of its primitives, named by the declarations as
//! written, and `Primitive` holds one variant per namespace, so each namespace's primitives are
//! implemented and matched apart.

use proc_macro2::Ident;
use quote::quote;
use vun_build::layout::SEPARATOR;
use vun_diag::{DiagId, Msg};
use vun_syntax::literal::Literal;

use crate::generate::{Access, Emit, Generator, Module, Read, Source};
use crate::items;
use crate::names::{docs, ident, pascal, raw};

pub(crate) const GENERATOR: Generator = Generator {
    source: "lib/lang",
    output: "compiler/lang/primitives/src/generated",
    emit: Emit::Rust(emit),
    keeps: &[],
};

/// The module of the enum over every namespace's primitives.
const PRIMITIVE_MODULE: &str = "primitive";

const EMPTY: DiagId = DiagId::new("generate.primitives.empty");

/// The primitives of one namespace.
struct Namespace {
    /// The binding path: `lang.mem`.
    path: Vec<String>,
    /// The module name: `mem`, or the package's name for its root namespace.
    stem: String,
    /// Each primitive's name and doc lines.
    items: Vec<(String, Vec<String>)>,
}

impl Namespace {
    fn of(source: &Source<'_>) -> Option<Self> {
        let namespace = source.namespace()?;
        let holes = source.decls.iter().filter(|d| d.value == Literal::Hole);
        let items: Vec<_> = holes.map(|d| (d.name.to_owned(), docs(&d.doc))).collect();
        let stem = Module::nested(&namespace).unwrap_or_else(|| namespace[0].clone());
        (!items.is_empty()).then_some(Self { path: namespace, stem, items })
    }

    /// Its path as written: `lang.mem`.
    fn written(&self) -> String {
        self.path.join(SEPARATOR)
    }

    fn module(&self) -> Ident {
        ident(&self.stem)
    }

    fn name(&self) -> Ident {
        pascal(&self.stem)
    }

    /// Its module: the enum, its namespace, its list and each variant's name.
    fn emit(&self) -> Module {
        let (module, name) = (self.module(), self.name());
        let doc = format!(" The primitives declared in `{}`.", self.written());
        let variants = self.items.iter().map(|(item, doc)| {
            let variant = raw(item);
            quote! { #(#[doc = #doc])* #variant }
        });
        let all = self.items.iter().map(|(item, _)| raw(item));
        let names = self.items.iter().map(|(item, _)| {
            let variant = raw(item);
            quote! { Self::#variant => #item }
        });
        let path: Vec<&str> = self.path.iter().map(String::as_str).collect();
        let namespace = items::Handles::rust_path(&path);
        let handles = self.items.iter().map(|(item, _)| {
            let segments: Vec<&str> = path.iter().copied().chain([item.as_str()]).collect();
            let (variant, handle) = (raw(item), items::Handles::rust_path(&segments));
            quote! { Self::#variant => #handle }
        });
        let items = quote! {
                #[doc = #doc]
                #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_codec::Encode, vun_codec::Decode)]
                #[allow(non_camel_case_types, reason = "variants are named as their declarations")]
                pub enum #name { #(#variants),* }

                impl #name {
                    pub const NAMESPACE: vun_items::Item = #namespace;
                    pub const ALL: &[Self] = &[#(Self::#all),*];

                    /// The declaration's name.
                    #[must_use]
                    pub const fn name(self) -> &'static str {
                        match self { #(#names),* }
                    }

                    /// The declaration.
                    #[must_use]
                    pub const fn item(self) -> vun_items::Item {
                        match self { #(#handles),* }
                    }
                }
        };
        Module { name: module.to_string(), access: Access::Path, items, dir: None }
    }
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Module>> {
    let namespaces: Vec<Namespace> = sources.iter().filter_map(Namespace::of).collect();
    if namespaces.is_empty() {
        let msg = Msg::new().text("declares no primitive: no declaration's value is ").code("_");
        return Err(GENERATOR.error(EMPTY, "lang", msg));
    }
    let mut modules: Vec<Module> = namespaces.iter().map(Namespace::emit).collect();
    let variants = namespaces.iter().map(|n| {
        let (module, name, doc) = (n.module(), n.name(), format!(" `{}`", n.written()));
        quote! { #[doc = #doc] #name(#module::#name) }
    });
    let all = namespaces.iter().map(|n| {
        let (module, name) = (n.module(), n.name());
        quote! { #module::#name::ALL.iter().map(|&p| Self::#name(p)) }
    });
    let namespace = namespaces.iter().map(|n| {
        let (module, name) = (n.module(), n.name());
        quote! { Self::#name(_) => #module::#name::NAMESPACE }
    });
    let names = namespaces.iter().map(|n| {
        let name = n.name();
        quote! { Self::#name(p) => p.name() }
    });
    let items = namespaces.iter().map(|n| {
        let name = n.name();
        quote! { Self::#name(p) => p.item() }
    });
    let primitive = quote! {
        /// A declaration of `lang` whose value is `_`, provided by the compiler, by namespace.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_codec::Encode, vun_codec::Decode)]
        pub enum Primitive { #(#variants),* }

        impl Primitive {
            /// Every primitive, namespace by namespace.
            pub fn all() -> impl Iterator<Item = Self> {
                std::iter::empty() #(.chain(#all))*
            }

            /// The namespace declaring it.
            #[must_use]
            pub const fn namespace(self) -> vun_items::Item {
                match self { #(#namespace),* }
            }

            /// The declaration.
            #[must_use]
            pub const fn item(self) -> vun_items::Item {
                match self { #(#items),* }
            }

            /// The declaration's name.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { #(#names),* }
            }
        }
    };
    modules.push(Module::sibling(PRIMITIVE_MODULE.to_owned(), &primitive));
    Ok(modules)
}
