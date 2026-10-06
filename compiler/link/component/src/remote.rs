//! The host's side: a linker that is a component, called through the boundary.

use std::borrow::Cow;

use vun_codec::Malformed;
use vun_component::{Component, Fault};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_link::{LinkPlan, Linker, LinkerInfo};
use vun_target::{Run, TargetSpec};
use vun_trace::tracing::debug;

use crate::{Call, Linkers, Reply};

/// A linker served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    info: &'static LinkerInfo,
}

impl Remote {
    /// The linker a component serves, its facts asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let info = Self::ask(&component, &Call::Info, Reply::info)?;
        debug!(target: "vun_component", linker = %info.id, library = ?component.path(), "opened a linker component");
        Ok(Self { component, info: Box::leak(Box::new(info)) })
    }

    fn ask<T>(
        component: &Component,
        call: &Call<'_>,
        read: impl FnOnce(Reply) -> Result<T, Malformed>,
    ) -> Result<T, Fault> {
        read(component.call::<Linkers>(call, &|()| ())?).map_err(Fault::Malformed)
    }
}

impl Linker for Remote {
    fn info(&self) -> &LinkerInfo {
        self.info
    }

    fn runs(&self, target: &TargetSpec) -> Vec<Run> {
        Self::ask(&self.component, &Call::Runs(Cow::Borrowed(target)), Reply::runs)
            .unwrap_or_else(|fault| panic!("linker component {}: {fault}", self.info.id))
    }

    fn link(&self, plan: &LinkPlan, target: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
        let call = Call::Link { plan: Cow::Borrowed(plan), target: Cow::Borrowed(target) };
        let (theirs, failed) = Self::ask(&self.component, &call, Reply::linked)
            .map_err(|fault| fault.report(self.info.id.0, diag))?;
        let failure = diag.absorb(&theirs);
        Fault::conclude(self.info.id.0, failed, failure, diag)
    }
}
