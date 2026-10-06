//! Language features (spec 12, section 2): a declaration marked `#[gate.behind(feature)]` is
//! usable from its own package and from the packages that enable the feature.

use vun_hir::db::Package;
use vun_items::lang::gate_Feature as feature;
use vun_primitives::Primitive;
use vun_primitives::gate::Gate;
use vun_sema_ty::item::Site;
use vun_target::Feature;

use crate::attributes::Attributed;
use crate::env::Env;
use crate::target::{Read, Variants};

impl Variants for Feature {
    const VARIANTS: &'static [(vun_items::Item, Self)] = &[(feature::deferred, Self::Deferred)];
}

impl Env<'_, '_> {
    /// The feature the declaration `site` is behind, if it is behind one this file's package does
    /// not enable and does not declare it.
    #[must_use]
    pub fn disabled(&self, site: &Site) -> Option<Feature> {
        let Site::Decl(decl) = site else { return None };
        if decl.file.package == self.file.package {
            return None;
        }
        let attributes = decl.attributes(self.cx)?;
        let behind = Feature::read(attributes.get(Primitive::Gate(Gate::behind))?.first()?).ok()?;
        let enabled = &self.cx.get::<Package>(&self.file.package).features;
        (!enabled.contains(behind)).then_some(behind)
    }
}

#[cfg(test)]
mod tests;
