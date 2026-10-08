//! Derived witnesses (spec 02, section 3): an interface marked `#[derive(f)]` gives a struct, an
//! enum, a tuple or a record with no impl of it the witness `f(T)`, when every part of it, every
//! field and the payload of every variant, has the interface too. `f` builds the witness by reflection, as
//! `lang.ops.derive_eq` does for `lang.ops.Eq`.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Member, each_entry};
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::call::{Func, Instance};
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{NominalKind, Ty, TyKind, Value};

use super::{Contract, ImplDecl, Witness};
use crate::decl::DeclValue;
use crate::env::Env;
use crate::eval::Call;
use crate::member::Members;

/// An interface marked `#[derive(f)]`, and the function `f` building the witness of a type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Derive {
    /// The interface: its function from a type to the requirement struct.
    pub interface: Func,
    /// `f`, from a type to the struct that is its witness.
    pub with: Value,
    /// The declaration of the interface.
    pub decl: DeclRef,
    /// The attribute, where the witness is reported.
    pub attr: OriginId,
}

/// The interfaces a file marks `#[derive(f)]`, in source order.
pub struct Derives;

impl Query for Derives {
    const NAME: &'static str = "derives";
    type Key = FileRef;
    type Value = Arc<[Derive]>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let Some(env) = Env::of(cx, file) else { return Arc::from([]) };
        let hir = &env.parsed.hir;
        let mut derives = Vec::new();
        for (_, ns) in hir.namespaces.iter() {
            each_entry(&ns.members, &mut |member| {
                let Member::Decl(decl) = member else { return };
                for &attr in &hir.decls[*decl].attrs {
                    let Some(with) = env.derive_attr(attr) else { continue };
                    let decl = DeclRef::new(file.clone(), hir, *decl);
                    let Ok(Value::Interface(interface)) = cx.get::<DeclValue>(&decl) else { continue };
                    derives.push(Derive { interface, with, decl, attr: hir.exprs[attr].origin });
                }
            });
        }
        derives.into()
    }

    fn cycle(_: &Cx<'_>, _: &FileRef, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

impl Env<'_, '_> {
    /// The function `f` of `#[derive(f)]` at `attr`; `None` for any other attribute.
    fn derive_attr(&self, attr: ExprId) -> Option<Value> {
        let ExprKind::Call { callee, args } = &self.parsed.hir.exprs[attr].kind else { return None };
        let Ok(Value::Primitive(Primitive::Lang(Lang::derive))) = self.eval(*callee) else { return None };
        let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
        self.eval(*value).ok()
    }
}

impl Witness {
    /// The parts of `ty` a derived witness is built from: of a struct or an enum, of a tuple (the
    /// unit among them) and of a record.
    fn parts(cx: &Cx<'_>, ty: Ty) -> Option<Vec<Ty>> {
        match &*ty {
            TyKind::Nominal(nominal) if nominal.kind != NominalKind::Empty => {
                let members = cx.get::<Members>(&ty)?;
                if !members.complete {
                    return None;
                }
                let fields = members.fields.iter().map(|f| f.ty.as_ref());
                let payloads = members.variants.iter().flat_map(|v| v.payload.iter().map(Result::as_ref));
                fields.chain(payloads).map(|part| part.ok().copied()).collect::<Option<_>>()
            }
            TyKind::Tuple(items) => Some(items.clone()),
            TyKind::Record(fields) => Some(fields.iter().map(|(_, ty)| *ty).collect()),
            _ => None,
        }
    }

    /// Whether an interface can be derived for `ty`: `has` holds for every part of it.
    pub(super) fn derivable(cx: &Cx<'_>, ty: Ty, has: impl Fn(Ty) -> bool) -> bool {
        Self::parts(cx, ty).is_some_and(|parts| parts.into_iter().all(has))
    }

    /// The witness `derive` gives `ty`, once [`Self::derivable`] holds.
    pub(super) fn derived(cx: &Cx<'_>, derive: &Derive, ty: Ty) -> Option<Self> {
        let env = Env::of(cx, &derive.decl.file)?;
        let witness = env.apply(derive.with.clone(), vec![Value::Type(ty)]).ok()?.as_type()?;
        let call = Instance { f: derive.interface.clone(), args: vec![Value::Type(ty)] };
        let requirements = cx.get::<Call>(&call).ok()?.as_type()?;
        let contract = Contract { interface: derive.interface.clone(), requirements };
        let decl = ImplDecl {
            decl: derive.decl.clone(),
            attr: derive.attr,
            contract: Some(contract),
            holed: None,
            target: Value::Type(ty),
        };
        Some(Self { decl, ty: witness, requirements: Some(requirements) })
    }
}
