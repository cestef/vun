//! The target a session compiles for (spec 13, section 7). `--target` names a declaration of the
//! package `targets`, such as `bare.thumbv7em.eabihf`; without it, `targets.hosts` gives the
//! running machine's. It is chosen when the first file is named, and set as the query input every
//! width and byte order is read from, so nothing assumes one.

use std::sync::Arc;

use vun_command::{Args, Param};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_hir::db::{FileRef, Package};
use vun_query::Sharing;
use vun_sema::target::{Choice, Chosen, Target, TargetOf, TargetPath, Targets};
use vun_target::TargetSpec;
use vun_trace::tracing;

use crate::Session;

pub const TARGET: Param = Param::option("target")
    .about("the target to compile for: a path in the package `targets`, such as `linux.x86_64.kernel`")
    .env("VUN_TARGET");

/// Applies `--target` to the session.
///
/// # Errors
/// Never: a path naming no target is reported when the target is chosen.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    if let Some(path) = args.value(&TARGET) {
        session.choice = Choice::Path(TargetPath::parse(path));
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
        // A package says what it is written for; naming a target overrides it.
        if self.choice == Choice::Host {
            let declared = self.db.get::<Package>(&key.package).target.clone();
            if !declared.is_empty() {
                self.choice = Choice::Path(TargetPath(declared));
            }
        }
        let chosen = self.db.get::<Chosen>(&(key.clone(), self.choice.clone()));
        let target = match chosen {
            Ok(declared) => {
                let spec = &declared.spec;
                tracing::info!(arch = %spec.arch, cpu = %spec.cpu, os = %spec.os, "target chosen");
                let spec = self.equip(key, spec.clone());
                let target = Target::of(&declared);
                let current = self.db.get::<TargetOf>(&());
                assert!(
                    current.as_deref().is_none_or(|t| *t == target),
                    "sessions sharing queries compile for one target (`Shared` gives one database per package)"
                );
                // Shared memos are kept by the target chosen, which they may read.
                self.db.set_as::<TargetOf>(&(), Some(Arc::new(target)), Sharing::Chosen);
                Ok(spec)
            }
            Err(why) => {
                let (msg, help) = self.unnamed(key);
                Err(self
                    .diag
                    .error(Diagnostic::error(errors::TARGET_UNCHOSEN, msg).caused_by(&why).help(help)))
            }
        };
        self.target = Some(target.clone());
        target
    }

    /// Whether choosing the target failed, which is reported: what reads it is unknown.
    #[must_use]
    pub fn untargeted(&self) -> bool {
        matches!(self.target, Some(Err(_)))
    }

    /// What reports that the target the session was asked for is none: the path `--target` gave,
    /// and the target it is closest to, else where the targets are listed.
    fn unnamed(&self, key: &FileRef) -> (Msg, Msg) {
        let Choice::Path(path) = &self.choice else {
            let help = Msg::new().text("choose one with ").code("--target").text(", of those ");
            return (Msg::new().text("no target is chosen"), help.code("vun target list").text(" lists"));
        };
        let named: Vec<String> = self.db.get::<Targets>(key).iter().map(|l| l.path.to_string()).collect();
        let given = path.to_string();
        let help = vun_suggest::closest(&given, named.iter().map(String::as_str)).map_or_else(
            || Msg::new().text("the targets are listed by ").code("vun target list"),
            Msg::did_you_mean,
        );
        (Msg::new().text("no target is named ").code(given), help)
    }
}
