//! The levels lints run at (spec 10, section 4): a package's build records pairs of a lint or a
//! group of lints and a level (`b.lints(.{ .{ lint, level } })`), read from the steps of the
//! package's plan, and the command line overrides them (`--lint lint=level`), which is the input
//! [`LintLevelsOf`]. A pair naming the lint wins over one naming its group, the last written of
//! each wins, the command line wins over the build, and nothing lowers `forbid`.

use vun_diag::DiagId;
use vun_hir::db::{FileRef, Package, PackageId};
use vun_items::std as std_items;
use vun_query::{Cx, Query};
use vun_sema_ty::Value;

use super::tables::Level;
use crate::env::Env;
use crate::plan::Planned;

/// The lint levels the command line gives, by the binding path of the lint or group they name,
/// in the order they were written. The driver sets it from `--lint`.
pub struct LintLevelsOf;

impl Query for LintLevelsOf {
    const NAME: &'static str = "lint_levels";
    const INPUT: bool = true;
    type Key = ();
    type Value = LintLevels;

    fn compute(_: &Cx<'_>, (): &()) -> Self::Value {
        LintLevels::default()
    }
}

/// The lint levels the command line gives, in the order they were written, and the packages they
/// apply to: the code the command names, never the libraries it depends on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LintLevels {
    pairs: Vec<(DiagId, Level)>,
    packages: Vec<PackageId>,
}

impl LintLevels {
    /// The levels of the pairs, in order, applying to no package until one is named.
    #[must_use]
    pub fn of(pairs: impl IntoIterator<Item = (DiagId, Level)>) -> Self {
        Self { pairs: pairs.into_iter().collect(), packages: Vec::new() }
    }

    /// The same levels, applying to `packages`.
    #[must_use]
    pub fn scoped_to(mut self, packages: impl IntoIterator<Item = PackageId>) -> Self {
        self.packages = packages.into_iter().collect();
        self.packages.sort();
        self.packages.dedup();
        self
    }

    /// Each pair, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&DiagId, Level)> {
        self.pairs.iter().map(|(id, level)| (id, *level))
    }

    /// Whether none is given.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Whether the levels apply to `package`.
    #[must_use]
    pub fn applies(&self, package: PackageId) -> bool {
        self.packages.contains(&package)
    }

    /// The level given to `id` by the last pair naming it, if any.
    fn last(&self, id: &DiagId) -> Option<Level> {
        self.pairs.iter().rev().find(|(named, _)| named == id).map(|&(_, level)| level)
    }
}

/// The levels a package's build records, and the command line overrides, in the order each was
/// written.
#[derive(Default)]
pub(super) struct Configured {
    plan: Vec<(Value, Level)>,
    /// The command line levels, when they apply to the file's package.
    cli: Option<LintLevels>,
}

impl Configured {
    /// What the plan of the package of the file `env` reads configures, and what the command
    /// line configures for its package; nothing for a plan that does not run, which checking its
    /// manifest reports.
    pub(super) fn of(env: &Env<'_, '_>) -> Self {
        let cli = env.cx.get::<LintLevelsOf>(&());
        let cli = cli.applies(env.file.package).then_some(cli);
        Self { plan: Self::plan(env), cli }
    }

    /// The pairs of a lint or a group and a level the build of the package of the file `env`
    /// records, in order.
    fn plan(env: &Env<'_, '_>) -> Vec<(Value, Level)> {
        let Some(manifest) = env.cx.get::<Package>(&env.file.package).manifest else {
            return Vec::new();
        };
        let key = FileRef { package: env.file.package, path: manifest };
        let Some(Ok(plan)) = env.cx.get::<Planned>(&key) else { return Vec::new() };
        let recorded = plan.actions.iter().filter_map(|action| match action {
            Value::Variant { name, payload, .. } if *name == std_items::build_Action::lints.symbol() => {
                payload.first()
            }
            _ => None,
        });
        recorded
            .flat_map(|levels| {
                let Value::Any { value, .. } = levels else { return Vec::new() };
                let (Value::Tuple(entries) | Value::Array(entries)) = value.as_ref() else {
                    return Vec::new();
                };
                entries.iter().filter_map(Self::pair).collect()
            })
            .collect()
    }

    /// The lint or group and the level of `entry`, a tuple whose items after the level are the
    /// lint's options.
    fn pair(entry: &Value) -> Option<(Value, Level)> {
        let Value::Tuple(items) = entry else { return None };
        let [target, level, ..] = items.as_slice() else { return None };
        Some((target.clone(), Level::of(level)?))
    }

    /// The level of the lint `lint`, named `id`, of the group `group`, named `group_id`, at
    /// `level` unless configured. `forbid` from any of these is final.
    pub(super) fn level(
        &self,
        lint: &Value,
        id: &DiagId,
        group: Option<&Value>,
        group_id: &DiagId,
        level: Level,
    ) -> Level {
        let plan =
            |target: Option<&Value>| self.plan.iter().rev().find(|(t, _)| Some(t) == target).map(|&(_, l)| l);
        let sources = [
            self.cli.as_ref().and_then(|cli| cli.last(id)),
            self.cli.as_ref().and_then(|cli| cli.last(group_id)),
            plan(Some(lint)),
            plan(group),
            Some(level),
        ];
        if sources.into_iter().flatten().any(|level| level == Level::Forbid) {
            return Level::Forbid;
        }
        sources.iter().flatten().next().copied().unwrap_or(Level::Allow)
    }
}
