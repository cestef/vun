//! The target a session compiles for (spec 13, section 7). `--target` names a declaration of the
//! package `targets`, such as `bare.cortex_m4f`; without it, `targets.hosts` gives the
//! running machine's. It is chosen when the first file is named, and set as the query input every
//! width and byte order is read from, so nothing assumes one.

use std::sync::Arc;

use vun_command::{Args, Param, ParamKind};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_grammar::PATH_SEPARATOR;
use vun_hir::db::FileRef;
use vun_intern::Symbol;
use vun_sema::target::{Choice, Chosen, Target, TargetOf};
use vun_target::TargetSpec;
use vun_trace::tracing;

use crate::Session;

pub const TARGET: Param = Param {
    name: "target",
    short: None,
    about: "the target to compile for: a path in the package `targets`, such as `linux.x86_64`",
    kind: ParamKind::Option,
};

/// Applies `--target` to the session.
///
/// # Errors
/// Never: a path naming no target is reported when the target is chosen.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    if let Some(path) = args.value(&TARGET) {
        session.choice = Choice::Path(path.split(PATH_SEPARATOR).map(Symbol::intern).collect());
    }
    Ok(())
}

impl Session {
    /// The target, chosen from the package graph of `key` the first time; reported once when
    /// none is.
    pub(crate) fn target_for(&mut self, key: &FileRef) -> Result<TargetSpec, ErrorGuaranteed> {
        if let Some(target) = &self.target {
            return target.clone();
        }
        let chosen = self.db.get::<Chosen>(&(key.clone(), self.choice.clone()));
        let target = match chosen {
            Ok(spec) => {
                tracing::info!(arch = %spec.arch, cpu = %spec.cpu, os = %spec.os, "target chosen");
                let target = Target::of(&spec);
                let current = self.db.get::<TargetOf>(&());
                assert!(
                    current.as_deref().is_none_or(|t| *t == target),
                    "sessions sharing queries compile for one target (`Shared` gives one database per package)"
                );
                self.db.set::<TargetOf>(&(), Some(Arc::new(target)));
                Ok(spec)
            }
            Err(why) => {
                let msg = Msg::new().text("no target is chosen");
                Err(self.diag.error(Diagnostic::error(errors::TARGET_UNCHOSEN, msg).caused_by(&why)))
            }
        };
        self.target = Some(target.clone());
        target
    }
}
