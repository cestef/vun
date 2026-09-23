//! The primitives of `overt-sema`, from `lib/lang`: every declaration of the package's namespace
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

use overt_build::layout::SEPARATOR;
use overt_diag::{DiagId, Msg};
use overt_syntax::literal::Literal;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::generate::{self, Generator, Read, Source};
use crate::names::{docs, ident, pascal};

pub(crate) const GENERATOR: Generator =
    Generator { source: "lib/lang", output: "compiler/front/sema/src/primitives.rs", emit };

const EMPTY: DiagId = DiagId::new("xtask.primitives.empty");

/// Joins the segments of a nested namespace in a module name.
const JOIN: &str = "_";

/// The primitives of one namespace.
struct Namespace {
    /// The binding path: `lang.mem`.
    path: String,
    /// The module name: `mem`, or the package's name for its root namespace.
    stem: String,
    /// Each primitive's name and doc lines.
    items: Vec<(String, Vec<String>)>,
}

/// A declaration name as a Rust identifier, raw when it is a keyword: `r#type`.
fn variant(name: &str) -> Ident {
    syn::parse_str::<Ident>(name).unwrap_or_else(|_| Ident::new_raw(name, Span::call_site()))
}

impl Namespace {
    fn of(source: &Source<'_>) -> Option<Self> {
        let namespace = source.namespace()?;
        let holes = source.decls.iter().filter(|d| d.value == Literal::Hole);
        let items: Vec<_> = holes.map(|d| (d.name.to_owned(), docs(&d.doc))).collect();
        let stem = if namespace.len() == 1 { namespace[0].clone() } else { namespace[1..].join(JOIN) };
        (!items.is_empty()).then(|| Self { path: namespace.join(SEPARATOR), stem, items })
    }

    fn module(&self) -> Ident {
        ident(&self.stem)
    }

    fn name(&self) -> Ident {
        pascal(&self.stem)
    }

    /// Its module: the enum, its namespace, its list and each variant's name.
    fn emit(&self) -> TokenStream {
        let (module, name, path) = (self.module(), self.name(), &self.path);
        let doc = format!(" The primitives declared in `{path}`.");
        let variants = self.items.iter().map(|(item, doc)| {
            let variant = variant(item);
            quote! { #(#[doc = #doc])* #variant }
        });
        let all = self.items.iter().map(|(item, _)| variant(item));
        let names = self.items.iter().map(|(item, _)| {
            let variant = variant(item);
            quote! { Self::#variant => #item }
        });
        quote! {
            pub mod #module {
                #[doc = #doc]
                #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
                #[allow(non_camel_case_types, reason = "variants are named as their declarations")]
                pub enum #name { #(#variants),* }

                impl #name {
                    pub const NAMESPACE: &str = #path;
                    pub const ALL: &[Self] = &[#(Self::#all),*];

                    /// The declaration's name.
                    #[must_use]
                    pub const fn name(self) -> &'static str {
                        match self { #(#names),* }
                    }
                }
            }
        }
    }
}

fn emit(sources: &[Source<'_>]) -> Read<TokenStream> {
    let namespaces: Vec<Namespace> = sources.iter().filter_map(Namespace::of).collect();
    if namespaces.is_empty() {
        let msg = Msg::new().text("declares no primitive: no declaration's value is ").code("_");
        return Err(generate::error(EMPTY, GENERATOR.source, "lang", msg));
    }
    let modules = namespaces.iter().map(Namespace::emit);
    let variants = namespaces.iter().map(|n| {
        let (module, name, doc) = (n.module(), n.name(), format!(" `{}`", n.path));
        quote! { #[doc = #doc] #name(#module::#name) }
    });
    let all = namespaces.iter().flat_map(|n| {
        let (module, name) = (n.module(), n.name());
        n.items.iter().map(move |(item, _)| {
            let item = variant(item);
            quote! { Self::#name(#module::#name::#item) }
        })
    });
    let namespace = namespaces.iter().map(|n| {
        let (module, name) = (n.module(), n.name());
        quote! { Self::#name(_) => #module::#name::NAMESPACE }
    });
    let names = namespaces.iter().map(|n| {
        let name = n.name();
        quote! { Self::#name(p) => p.name() }
    });
    Ok(quote! {
        #(#modules)*

        /// A declaration of `lang` whose value is `_`, provided by the compiler, by namespace.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Primitive { #(#variants),* }

        impl Primitive {
            pub const ALL: &[Self] = &[#(#all),*];

            /// The binding path of the namespace declaring it.
            #[must_use]
            pub const fn namespace(self) -> &'static str {
                match self { #(#namespace),* }
            }

            /// The declaration's name.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { #(#names),* }
            }
        }
    })
}
