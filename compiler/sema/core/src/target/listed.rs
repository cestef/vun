//! Every target of the package `targets`, with the path `--target` names it by (spec 13,
//! section 7): the public members of its namespaces, nested ones walked, whose values read as a
//! `lang.target.Spec`.

use vun_hir::db::{Exports, FileRef, Namespace, NamespaceAt};
use vun_query::{Cx, Query};
use vun_sema_ty::item::Site;

use super::{Declared, Read, TargetPath};
use crate::env::Env;

/// A target and its path in the package `targets`, as [`super::Choice::Path`] holds it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, vun_codec::Encode, vun_codec::Decode)]
pub struct Listed {
    pub path: TargetPath,
    pub target: Declared,
}

/// The targets a file sees, in the order their namespaces declare them.
pub struct Targets;

impl Query for Targets {
    const NAME: &'static str = "targets";
    type Key = FileRef;
    type Value = Vec<Listed>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let Some(env) = Env::of(cx, file) else { return Vec::new() };
        let Ok(Site::Namespace(root)) = env.site(vun_items::targets) else { return Vec::new() };
        let mut listed = Vec::new();
        Listed::walk(cx, &env, &root, &mut listed);
        listed
    }
}

impl Listed {
    fn walk(cx: &Cx<'_>, env: &Env<'_, '_>, ns: &Namespace, listed: &mut Vec<Self>) {
        for &name in cx.get::<Exports>(ns).iter() {
            let mut names = ns.path.clone();
            names.push(name);
            let path = TargetPath(names);
            if let Ok(inner) = cx.get::<NamespaceAt>(&(ns.package, path.from_package().collect())) {
                Self::walk(cx, env, &inner, listed);
            } else if let Ok(Ok(target)) = env.path(path.from_package()).map(|v| Declared::read(&v)) {
                listed.push(Self { path, target });
            }
        }
    }
}
