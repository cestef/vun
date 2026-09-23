//! Spreads in a checked file (spec 01, section 8): their paths, the names they select, and the
//! names a whole spread brings into its namespace.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use overt_build::layout::SEPARATOR;
use overt_diag::{ChildKind, Diagnostic, ErrorGuaranteed, Label, Msg, errors};
use overt_hir::resolve::{Outside, Res, Resolution};
use overt_hir::tree::{Hir, Member, NsId, Spread, each_entry};
use overt_intern::Symbol;
use overt_suggest::closest;
use overt_syntax::visibility::Visibility;

use super::{Namespace, PackageKey, Scope, Unfound};
use crate::Session;

/// What spreads bring, by the namespace they are spread into.
#[derive(Default)]
pub struct Spreads {
    /// The names whole spreads bring.
    pub brought: HashMap<NsId, HashMap<Symbol, Namespace>>,
    /// Namespaces with a spread whose target could not be read, which is reported.
    #[expect(clippy::zero_sized_map_values, reason = "each value is the proof of a report")]
    pub poisoned: HashMap<NsId, ErrorGuaranteed>,
}

/// What a file sees from outside itself: names spread into its namespaces, then its package's.
pub struct FileScope<'s> {
    pub package: &'s Scope,
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
                let brought = self.spreads.brought.get(&ns);
                brought.map(|names| names.keys().copied().collect()).unwrap_or_default()
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

impl Session {
    /// Checks every spread of `hir` and collects what the whole ones bring.
    pub fn spreads(&mut self, package: PackageKey, hir: &Hir, resolution: &Resolution) -> Spreads {
        let mut spreads = Spreads::default();
        for (ns, namespace) in hir.namespaces.iter() {
            let mut found = Vec::new();
            each_entry(&namespace.members, &mut |m| {
                if let Member::Spread(s) = m {
                    found.push(s);
                }
            });
            for s in found {
                match self.spread_target(package, hir, resolution, s) {
                    Ok((path, target)) => {
                        self.spread(hir, ns, s, &path, &target, spreads.brought.entry(ns).or_default());
                    }
                    Err(Some(reported)) => {
                        spreads.poisoned.insert(ns, reported);
                    }
                    Err(None) => {}
                }
            }
        }
        spreads
    }

    /// The namespace `s` spreads, and its path as written; or the proof that why not is reported,
    /// unless its first name is found nowhere, which resolution reports.
    fn spread_target(
        &mut self,
        package: PackageKey,
        hir: &Hir,
        resolution: &Resolution,
        s: &Spread,
    ) -> Result<(String, Namespace), Option<ErrorGuaranteed>> {
        let origin = hir.exprs[s.path].origin;
        let Some((root, names)) = hir.path(s.path) else {
            let msg = Msg::new().text("a spread takes a namespace path, such as ").code("..std.mem");
            return Err(Some(
                self.diag.error(Diagnostic::error(errors::PACKAGE_MISSING_NAMESPACE, msg).at(origin, None)),
            ));
        };
        let text: Vec<&str> = names.iter().map(|&n| n.as_str()).collect();
        let text = text.join(SEPARATOR);
        let local = matches!(resolution.names[root], Some(Res::Binder(_)));
        let known = self.packages.scopes[&package].names.contains_key(&names[0]);
        match self.namespace_at(package, &names) {
            Ok(target) if !local => Ok((text, target)),
            Err(Unfound::Dep(reported)) => Err(Some(reported)),
            // A first name found nowhere is reported as unresolved.
            Err(Unfound::Root) if !known && !local => Err(None),
            found => {
                let mut d = Self::not_namespace(&text).at(origin, None);
                if local || found.is_err_and(|e| e == Unfound::Root) {
                    let help = Msg::new().text("a spread path starts at the package or a dependency");
                    d = d.child(ChildKind::Help, help);
                }
                Err(Some(self.diag.error(d)))
            }
        }
    }

    /// Checks the names `s` selects, or brings every export of `target` into `ns`.
    fn spread(
        &mut self,
        hir: &Hir,
        ns: NsId,
        s: &Spread,
        path: &str,
        target: &Namespace,
        brought: &mut HashMap<Symbol, Namespace>,
    ) {
        let Some(select) = &s.select else {
            let declared: Vec<_> = hir.members(ns);
            for name in self.exports(target) {
                let text = name.as_str().to_owned();
                let msg = Msg::new().code(&text).text(" is spread from ").code(path);
                let earlier = declared.iter().find(|&&b| hir.binders[b].name == name);
                if let Some(&b) = earlier {
                    let d = Diagnostic::error(errors::NAMES_DUPLICATE, msg.text(" and declared here"));
                    let mut d = d.at(s.origin, None);
                    d.labels.push(Label { origin: hir.binders[b].origin, msg: Some("declared here".into()) });
                    self.diag.error(d);
                } else if let Entry::Vacant(slot) = brought.entry(name) {
                    slot.insert(target.clone());
                } else {
                    let d = Diagnostic::error(errors::NAMES_DUPLICATE, msg.text(" and by another spread"));
                    self.diag.error(d.at(s.origin, None));
                }
            }
            return;
        };
        let members = self.members(target);
        for &b in select {
            let name = hir.binders[b].name;
            let text = name.as_str();
            let visibility = Visibility::of(text);
            let exists = members.contains(&name);
            if exists && visibility == Visibility::Public {
                continue;
            }
            let d = if exists {
                let msg = Msg::new().code(text).text(" is private to ").code(path);
                Diagnostic::error(errors::NAMES_PRIVATE, msg)
            } else {
                let msg = Msg::new().code(path).text(" has no member ").code(text);
                let d = Diagnostic::error(errors::NAMES_UNRESOLVED, msg);
                let public = members.iter().map(|&m| m.as_str());
                match closest(text, public.filter(|m| Visibility::of(m) == Visibility::Public)) {
                    Some(near) => d.child(ChildKind::Help, Msg::did_you_mean(near)),
                    None => d,
                }
            };
            self.diag.error(d.at(hir.binders[b].origin, None));
        }
    }
}
