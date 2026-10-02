//! Lints (spec 10, section 4): a file is checked with the lints its package and the packages it
//! depends on, `std` among them by default, define (`#[lint.define]`). Each runs the check it
//! states for a kind of node on each node of that kind in the file, at compile time, through
//! `lang.lint.reports`, a pure call the query engine caches; what it reports is shown at its
//! level, unless a `#[lint.allow]` around the node silences it.

mod defined;
mod levels;
mod literal;
mod nodes;
mod rule;
mod tables;
mod walk;

use std::sync::Arc;

use vun_diag::DiagCtxt;
use vun_hir::db::FileRef;
use vun_query::{Cx, Query};

pub use defined::{Checked, CheckedLints, DefinedLints};
pub use levels::{LintLevels, LintLevelsOf};
pub use tables::Level;

use crate::env::Env;
use nodes::Node;
use rule::Lint;

/// What the lints a file is checked with report of it; `None` for a file the driver did not read.
pub struct Linted;

impl Query for Linted {
    const NAME: &'static str = "linted";
    type Key = FileRef;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let env = Env::of(cx, file)?;
        let nodes = Node::of(&env);
        let mut diag = DiagCtxt::new();
        let lints = Lint::visible(&env);
        vun_trace::tracing::debug!(file = %file.path, lints = lints.len(), nodes = nodes.len(), "linting");
        for lint in lints {
            let fired = lint.run(&env, &nodes, &mut diag);
            if lint.level == Level::Expect && !fired {
                diag.report(lint.unfulfilled(&env));
            }
        }
        Some(Arc::new(diag))
    }
}
