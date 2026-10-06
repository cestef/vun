//! The host's side: a backend that is a component, called through the boundary.

use std::borrow::Cow;

use vun_backend::{ArtifactSink, Backend, BackendInfo, Request};
use vun_codec::Malformed;
use vun_component::{Component, Fault};
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir::FuncId;
use vun_lir::unit::Unit;
use vun_lir_passes::legalize::Division;
use vun_target::TargetSpec;
use vun_trace::tracing::debug;

use crate::{Answer, Ask, Backends, Call, Emit, Reply};

/// A backend served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    info: &'static BackendInfo,
}

impl Remote {
    /// The backend a component serves, its facts asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let reply = component.call::<Backends>(&Call::Info, &|ask| Self::unasked(&ask))?;
        let info = reply.info().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", backend = %info.id, library = ?component.path(), "opened a backend component");
        Ok(Self { component, info: Box::leak(Box::new(info)) })
    }

    /// What a call that has no source to ask of is answered with.
    fn unasked(ask: &Ask) -> Answer {
        panic!("a backend asked {ask:?} of a call with no source lines")
    }

    /// Calls the component for something no error can be returned from: a component that fails
    /// there is broken.
    fn infallible<T>(&self, call: &Call<'_>, read: impl FnOnce(Reply) -> Result<T, Malformed>) -> T {
        let reply = self
            .component
            .call::<Backends>(call, &|ask| Self::unasked(&ask))
            .and_then(|reply| read(reply).map_err(Fault::Malformed));
        reply.unwrap_or_else(|fault| panic!("backend component {}: {fault}", self.info.id))
    }
}

impl Backend for Remote {
    fn info(&self) -> &BackendInfo {
        self.info
    }

    fn emit(
        &self,
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let call = Call::Emit(Box::new(Emit {
            unit: Cow::Borrowed(request.unit),
            target: Cow::Borrowed(request.target),
            profile: request.profile,
            listing: request.listing,
        }));
        let reply = self.component.call::<Backends>(&call, &|ask| ask.answered_by(request.lines));
        let reply = reply.and_then(|reply| reply.emitted().map_err(Fault::Malformed));
        let (emitted, theirs, failed) = reply.map_err(|fault| fault.report(self.info.id.0, diag))?;
        let failure = diag.absorb(&theirs);
        for artifact in emitted.artifacts {
            out.put(artifact);
        }
        if !emitted.frames.is_empty() {
            out.frames(emitted.frames);
        }
        Fault::conclude(self.info.id.0, failed, failure, diag)
    }

    fn division(&self, target: &TargetSpec) -> Division {
        self.infallible(&Call::Division(Cow::Borrowed(target)), Reply::division)
    }

    fn frame_sizes(&self, unit: &Unit, report: &[u8]) -> Vec<(FuncId, u64)> {
        let call = Call::FrameSizes { unit: Cow::Borrowed(unit), report: Cow::Borrowed(report) };
        self.infallible(&call, Reply::frame_sizes)
    }
}
