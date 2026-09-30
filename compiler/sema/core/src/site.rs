//! Where a name written in a file is declared, as tools show it (spec 13, section 11): the binder
//! of the file it resolves to, or the declaration or namespace an outer name, a spread or a member
//! of a namespace or of a type's namespace reaches.

use vun_hir::resolve::Res;
use vun_hir::tree::{ExprKind, Name};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema_ty::Value;
use vun_sema_ty::item::{Declared, ExprRef, Site};

use crate::env::Env;
use crate::member::{Found, Members};

/// What a name leads to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Declaration {
    /// A local of its body or a parameter: the binder's origin.
    Local(OriginId),
    Site(Site),
}

/// The declaration the name, or the member of a namespace, `expr` is; `None` for another
/// expression, or a name that leads nowhere.
pub struct NameSite;

impl Query for NameSite {
    const NAME: &'static str = "name_site";
    type Key = ExprRef;
    type Value = Option<Declaration>;

    fn compute(cx: &Cx<'_>, expr: &ExprRef) -> Self::Value {
        let env = Env::of(cx, &expr.file)?;
        let hir = &env.parsed.hir;
        let id = expr.expr.id;
        match &hir.exprs[id].kind {
            ExprKind::Name(_) => match env.resolve(id) {
                Ok(site) => Some(Declaration::Site(site)),
                Err(_) => match env.parsed.resolution.names[id] {
                    Some(Res::Binder(b)) => Some(Declaration::Local(hir.binders[b].origin)),
                    _ => None,
                },
            },
            ExprKind::Member { object, name: Name::Plain(name) } => match env.eval(*object) {
                Ok(Value::Namespace(ns)) => cx.get::<Declared>(&(ns, *name)).map(Declaration::Site),
                // A declaration of a type's namespace, its own or one a spread brings.
                Ok(Value::Type(ty)) => match cx.get::<Members>(&ty)?.find(*name) {
                    Found::Decl(decl) => Some(Declaration::Site(Site::Decl(decl.decl.clone()))),
                    Found::Spread(site) => Some(Declaration::Site(site.clone())),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }
}
