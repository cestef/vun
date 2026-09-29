//! The operations of `lang.check` a runtime provides, by their link symbols: a program built
//! with checks that still imports one of them has a runtime without it.

use std::sync::Arc;

use vun_hir::db::PackageId;
use vun_intern::Symbol;
use vun_items::Item;
use vun_primitives::check::Check;
use vun_query::{Cx, Query};
use vun_sema::env::ItemSite;
use vun_sema::link::Linkable;
use vun_sema_ty::item::Site;

/// The operations of `lang.check` as the files of a package see them, each with its link symbol:
/// its primitives a runtime exports, the ones with a link symbol.
pub struct Operations;

impl Query for Operations {
    const NAME: &'static str = "check_operations";
    type Key = PackageId;
    type Value = Arc<Vec<(Item, Symbol)>>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        let linked = |item: Item| {
            let Ok(Site::Decl(decl)) = cx.get::<ItemSite>(&(*package, item)) else { return None };
            Some((item, decl.linked(cx)?.symbol))
        };
        Arc::new(Check::ALL.iter().map(|p| p.item()).filter_map(linked).collect())
    }
}
