//! Typed link symbols (spec 08, section 1, rule 3): a declaration the linker provides,
//! `#[link.symbol(s)] name: T = _;`, may be satisfied by a `#[link.export(s)]` of the same program,
//! whose value must then be of the declared type.

use std::sync::Arc;

use vun_hir::db::FileRef;
use vun_hir::tree::{Member, each_entry};
use vun_origin::OriginId;
use vun_query::{Cx, Cycle, Query};

use crate::Primitive;
use vun_intern::Symbol;

use crate::env::Env;
use crate::eval::Func;
use crate::item::DeclRef;
use crate::primitives::link::Link;

impl Func {
    /// The symbol this function is exported under (`#[link.export(s)]`), if any.
    #[must_use]
    pub fn export(&self, cx: &Cx<'_>) -> Option<Symbol> {
        let env = Env::defining(cx, &self.literal, self.within.as_deref())?;
        let attrs = env.attrs_of(self.literal.expr.id);
        let args = env.attribute(&attrs, Primitive::Link(Link::export))?;
        Some(Symbol::intern(&args.first()?.text()?))
    }
}

/// A declaration naming a link symbol: one the linker provides, or an export providing it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LinkDecl {
    pub decl: DeclRef,
    /// The attribute, where a mismatch is reported.
    pub attr: OriginId,
    /// `lang.link.symbol` or `lang.link.export`.
    pub side: Link,
    pub symbol: String,
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
                        if let Some(symbol) = named.as_deref().and_then(|args| args.first()?.text()) {
                            let decl = DeclRef::new(file.clone(), hir, *decl);
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
