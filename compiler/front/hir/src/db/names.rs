//! The name checks of a file that need its package (spec 01, section 8): its spreads, the names it
//! shares with its directory, and every name it takes from outside.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::{ChildKind, DiagCtxt, DiagCx, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_grammar::PATH_SEPARATOR;
use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_query::{Cx, Query};
use vun_suggest::closest;
use vun_syntax::visibility::Visibility;

use super::input::{FileRef, Package, PackageData, Unloaded};
use super::layout::Namespace;
use super::namespace::{Exports, Members, NamespaceAt, Unfound};
use super::parsed::{Parsed, ParsedFile};
use super::scope::{Scope, ScopeData};
use crate::resolve::{Outside, Res};
use crate::tree::{NsId, Spread};

/// What spreads bring into a file's namespaces.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Spreads {
    /// The names whole spreads bring.
    pub brought: BTreeMap<NsId, BTreeMap<Symbol, Namespace>>,
    /// Namespaces with a spread whose target could not be read, which is reported.
    #[expect(clippy::zero_sized_map_values, reason = "each value is the proof of a report")]
    pub poisoned: BTreeMap<NsId, ErrorGuaranteed>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct CheckedNames {
    pub spreads: Spreads,
    pub diag: DiagCtxt,
}

/// A file's names checked against its package; `None` for a file the driver did not read.
pub struct FileNames;

impl Query for FileNames {
    const NAME: &'static str = "file_names";
    type Key = FileRef;
    type Value = Option<Arc<CheckedNames>>;

    fn compute(cx: &Cx<'_>, key: &FileRef) -> Self::Value {
        let parsed = cx.get::<Parsed>(key)?;
        let scope = cx.get::<Scope>(&key.package);
        let mut check = Check {
            cx,
            key,
            parsed: &parsed,
            package: cx.get::<Package>(&key.package),
            scope: &scope,
            origins: OriginTable::global(),
            checked: CheckedNames::default(),
        };
        check.checked.diag.absorb(&scope.diag);
        check.spreads();
        check.children();
        let outside = FileScope { package: &scope, spreads: &check.checked.spreads };
        let mut dcx = DiagCx { diag: &mut check.checked.diag, origins: check.origins };
        parsed.resolution.check_outer(&parsed.hir, &outside, &mut dcx);
        Some(Arc::new(check.checked))
    }
}

/// What a file sees from outside itself: names spread into its namespaces, then its package's.
pub struct FileScope<'s> {
    pub package: &'s ScopeData,
    pub spreads: &'s Spreads,
}

impl Outside for FileScope<'_> {
    fn contains(&self, name: Symbol, ns: Option<NsId>) -> bool {
        match ns {
            Some(ns) => self.spreads.brought.get(&ns).is_some_and(|names| names.contains_key(&name)),
            None => self.package.names.contains_key(&name),
        }
    }

    fn names(&self, ns: Option<NsId>) -> Vec<Symbol> {
        match ns {
            Some(ns) => {
                self.spreads.brought.get(&ns).map(|names| names.keys().copied().collect()).unwrap_or_default()
            }
            None => self.package.names.keys().copied().collect(),
        }
    }

    fn poisoned(&self, ns: Option<NsId>) -> Option<ErrorGuaranteed> {
        match ns {
            Some(ns) => self.spreads.poisoned.get(&ns).copied(),
            None => self.package.poisoned,
        }
    }
}

struct Check<'a, 'db> {
    cx: &'a Cx<'db>,
    key: &'a FileRef,
    parsed: &'a ParsedFile,
    package: Arc<PackageData>,
    scope: &'a ScopeData,
    origins: &'static OriginTable,
    checked: CheckedNames,
}

impl Check<'_, '_> {
    /// Checks every spread and collects what the whole ones bring.
    fn spreads(&mut self) {
        let hir = &self.parsed.hir;
        for (ns, _) in hir.namespaces.iter() {
            for s in hir.spreads(ns) {
                match self.target(s) {
                    Ok((path, target)) => self.spread(ns, s, &path, &target),
                    Err(Some(reported)) => {
                        self.checked.spreads.poisoned.insert(ns, reported);
                    }
                    Err(None) => {}
                }
            }
        }
    }

    /// The namespace `s` spreads, and its path as written; or the proof that why not is reported,
    /// unless its first name is found nowhere, which resolution reports.
    fn target(&mut self, s: &Spread) -> Result<(String, Namespace), Option<ErrorGuaranteed>> {
        let hir = &self.parsed.hir;
        let origin = hir.exprs[s.path].origin;
        let Some((root, names)) = hir.path(s.path) else {
            let msg = Msg::new().text("a spread takes a namespace path, such as ").code("..std.mem");
            let d = Diagnostic::error(errors::PACKAGE_MISSING_NAMESPACE, msg).at(origin, None);
            return Err(Some(self.checked.diag.error(d)));
        };
        let text = names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(PATH_SEPARATOR);
        let local = matches!(self.parsed.resolution.names[root], Some(Res::Binder(_)));
        let known = self.scope.names.contains_key(&names[0]);
        match self.cx.get::<NamespaceAt>(&(self.key.package, names)) {
            Ok(target) if !local => Ok((text, target)),
            Err(Unfound::Dep(Unloaded::Absent(reports))) => Err(self.checked.diag.absorb(&reports)),
            Err(Unfound::Dep(Unloaded::Reported)) => Err(None),
            // A first name found nowhere is reported as unresolved.
            Err(Unfound::Root) if !known && !local => Err(None),
            found => {
                let mut d = Unfound::not_namespace(&text).at(origin, None);
                if local || found.is_err_and(|e| e == Unfound::Root) {
                    let help = Msg::new().text("a spread path starts at the package or a dependency");
                    d = d.child(ChildKind::Help, help);
                }
                Err(Some(self.checked.diag.error(d)))
            }
        }
    }

    /// Checks the names `s` selects, or brings every export of `target` into `ns`.
    fn spread(&mut self, ns: NsId, s: &Spread, path: &str, target: &Namespace) {
        let hir = &self.parsed.hir;
        let Some(select) = &s.select else {
            let declared = hir.members(ns);
            for &name in self.cx.get::<Exports>(target).iter() {
                let msg = Msg::new().code(name.as_str()).text(" is spread from ").code(path);
                let brought = self.checked.spreads.brought.entry(ns).or_default();
                let d = if let Some(&b) = declared.iter().find(|&&b| hir.binders[b].name == name) {
                    let d = Diagnostic::error(errors::NAMES_DUPLICATE, msg.text(" and declared here"));
                    d.at(s.origin, None).label(hir.binders[b].origin, Some("declared here".into()))
                } else if let Entry::Vacant(slot) = brought.entry(name) {
                    slot.insert(target.clone());
                    continue;
                } else {
                    Diagnostic::error(errors::NAMES_DUPLICATE, msg.text(" and by another spread"))
                        .at(s.origin, None)
                };
                self.checked.diag.error(d);
            }
            return;
        };
        let members = self.cx.get::<Members>(target);
        for &b in select {
            let name = hir.binders[b].name;
            let text = name.as_str();
            // Whether a restricted name reaches the spread depends on its attributes, which sema
            // reads (spec 01, section 7).
            if members.contains(&name) {
                continue;
            }
            let msg = Msg::new().code(path).text(" has no member ").code(text);
            let d = Diagnostic::error(errors::NAMES_UNRESOLVED, msg);
            let public =
                members.iter().map(|m| m.as_str()).filter(|m| Visibility::of(m) == Visibility::Public);
            let d = match closest(text, public) {
                Some(near) => d.child(ChildKind::Help, Msg::did_you_mean(near)),
                None => d,
            };
            self.checked.diag.error(d.at(hir.binders[b].origin, None));
        }
    }

    /// Reports the top-level names of the file that its directory also holds: both are members
    /// of one namespace.
    fn children(&mut self) {
        let (hir, package) = (&self.parsed.hir, &self.package);
        let (Some(root), Some(path)) = (hir.root, package.namespace_of(&self.key.path)) else { return };
        let children = package.children(&path);
        for b in hir.members(root) {
            let name = hir.binders[b].name;
            if children.contains(&name) {
                let dir = package.directory(&path).unwrap_or_default();
                let msg =
                    Msg::new().code(name.as_str()).text(" is declared here and also held in ").code(dir);
                self.checked
                    .diag
                    .error(Diagnostic::error(errors::NAMES_DUPLICATE, msg).at(hir.binders[b].origin, None));
            }
        }
    }
}
