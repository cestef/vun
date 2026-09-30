//! The levels a package's build gives lints (spec 10, section 4): `b.lints(.{ .{ lint, level } })`
//! records pairs of a lint or a group of lints and a level, read from the steps of the package's
//! plan. A pair naming the lint wins over one naming its group, the last written of each wins,
//! and nothing lowers `forbid`.

use vun_hir::db::{FileRef, Package};
use vun_items::std as std_items;
use vun_sema_ty::Value;

use super::tables::Level;
use crate::env::Env;
use crate::plan::Planned;

/// The pairs of a lint or a group and a level the build of a package records, in order.
#[derive(Default)]
pub(super) struct Configured(Vec<(Value, Level)>);

impl Configured {
    /// What the plan of the package of the file `env` reads configures; nothing for files
    /// outside any package, or a plan that does not run, which checking its manifest reports.
    pub(super) fn of(env: &Env<'_, '_>) -> Self {
        let Some(manifest) = env.cx.get::<Package>(&env.file.package).manifest else {
            return Self::default();
        };
        let key = FileRef { package: env.file.package, path: manifest };
        let Some(Ok(plan)) = env.cx.get::<Planned>(&key) else { return Self::default() };
        let recorded = plan.actions.iter().filter_map(|action| match action {
            Value::Variant { name, payload, .. } if *name == std_items::build_Action::lints.symbol() => {
                payload.first()
            }
            _ => None,
        });
        let pairs = recorded.flat_map(|levels| {
            let Value::Any { value, .. } = levels else { return Vec::new() };
            let (Value::Tuple(entries) | Value::Array(entries)) = value.as_ref() else { return Vec::new() };
            entries.iter().filter_map(Self::pair).collect()
        });
        Self(pairs.collect())
    }

    /// The lint or group and the level of `entry`, a tuple whose items after the level are the
    /// lint's options.
    fn pair(entry: &Value) -> Option<(Value, Level)> {
        let Value::Tuple(items) = entry else { return None };
        let [target, level, ..] = items.as_slice() else { return None };
        Some((target.clone(), Level::of(level)?))
    }

    /// The level of the lint `lint`, of the group `group`, at `level` unless configured.
    pub(super) fn level(&self, lint: &Value, group: Option<&Value>, level: Level) -> Level {
        let last =
            |target: Option<&Value>| self.0.iter().rev().find(|(t, _)| Some(t) == target).map(|&(_, l)| l);
        match last(Some(lint)).or_else(|| last(group)) {
            Some(_) if level == Level::Forbid => level,
            Some(configured) => configured,
            None => level,
        }
    }
}
