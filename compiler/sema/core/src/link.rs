//! Typed link symbols (spec 08, section 1, rule 3): a declaration the linker provides,
//! `#[link.symbol(s)] name: T = _;`, may be satisfied by a `#[link.export(s)]` of the same program,
//! whose value must then be of the declared type.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_hir::tree::{Member, each_entry};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::abi::Abi;
use vun_primitives::link::Link;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::call::Func;
use vun_sema_ty::item::DeclRef;

use crate::attributes::Attributed;
use crate::env::Env;

/// A function's linkage: the symbol it is exported as and its calling convention.
pub trait Exported {
    /// The symbol this function is exported under (`#[link.export(s)]`), if any.
    #[must_use]
    fn export(&self, cx: &Cx<'_>) -> Option<Symbol>;

    /// The calling convention its markers name (`#[abi.c]`), if any (spec 07, section 4).
    #[must_use]
    fn abi(&self, cx: &Cx<'_>) -> Option<Abi>;
}

impl Exported for Func {
    fn export(&self, cx: &Cx<'_>) -> Option<Symbol> {
        self.attributes(cx)?.text(Primitive::Link(Link::export))
    }

    fn abi(&self, cx: &Cx<'_>) -> Option<Abi> {
        self.attributes(cx)?.abi()
    }
}

/// How a declaration the linker provides is linked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Linked {
    /// `#[link.symbol(s)]`.
    pub symbol: Symbol,
    /// The calling convention its markers name.
    pub abi: Option<Abi>,
    /// The library the symbol comes from, `#[link.library(name)]`.
    pub library: Option<Symbol>,
}

/// A declaration's linkage: what provides it from outside the program.
pub trait Linkable {
    /// How the linker provides this declaration, if it does (`#[link.symbol(s)]`).
    #[must_use]
    fn linked(&self, cx: &Cx<'_>) -> Option<Linked>;
}

impl Linkable for DeclRef {
    fn linked(&self, cx: &Cx<'_>) -> Option<Linked> {
        let attributes = self.attributes(cx)?;
        Some(Linked {
            symbol: attributes.text(Primitive::Link(Link::symbol))?,
            abi: attributes.abi(),
            library: attributes.text(Primitive::Link(Link::library)),
        })
    }
}

/// A declaration naming a link symbol: one the linker provides, or an export providing it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct LinkDecl {
    pub decl: DeclRef,
    /// The attribute, where a mismatch is reported.
    pub attr: OriginId,
    /// `lang.link.symbol` or `lang.link.export`.
    pub side: Link,
    pub symbol: Symbol,
}

impl Env<'_, '_> {
    /// The declarations naming link symbols that this file sees: those of every file of its
    /// package and of the packages it depends on.
    #[must_use]
    pub fn links_seen(&self) -> Vec<LinkDecl> {
        self.impl_files(None).into_iter().flat_map(|f| self.cx.get::<Links>(&f).to_vec()).collect()
    }

    /// The most bytes of stack a call of the link symbol `symbol` takes, as the declarations of
    /// it this file sees state (`#[stack.max(n)]`): the largest, since each states it of the one
    /// function the linker provides.
    #[must_use]
    pub fn linked_stack(&self, symbol: Symbol) -> Option<u64> {
        let declared = self.links_seen().into_iter().filter(|l| l.side == Link::symbol && l.symbol == symbol);
        declared.filter_map(|l| l.decl.attributes(self.cx)?.stack_max()).max()
    }
}

/// The declarations of a file naming link symbols, in source order: in any namespace of it,
/// including `struct` bodies, such as those a runtime returns.
pub struct Links;

impl Query for Links {
    const NAME: &'static str = "links";
    type Key = FileRef;
    type Value = Arc<[LinkDecl]>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let Some(env) = Env::of(cx, file) else { return Arc::from([]) };
        let hir = &env.parsed.hir;
        let mut links = Vec::new();
        for (_, ns) in hir.namespaces.iter() {
            each_entry(&ns.members, &mut |member| {
                let Member::Decl(decl) = member else { return };
                for &attr in &hir.decls[*decl].attrs {
                    for side in [Link::symbol, Link::export] {
                        let named = env.attribute(&[attr], Primitive::Link(side));
                        if let Some(text) = named.as_deref().and_then(|args| args.first()?.text()) {
                            let (decl, symbol) =
                                (DeclRef::new(file.clone(), hir, *decl), Symbol::intern(&text));
                            links.push(LinkDecl { decl, attr: hir.exprs[attr].origin, side, symbol });
                        }
                    }
                }
            });
        }
        links.into()
    }

    fn cycle(_: &Cx<'_>, _: &FileRef, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}
