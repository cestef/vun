//! The host's side: a documenter that is a component, called through the boundary.

use std::borrow::Cow;

use vun_component::{Cause, Component, Fault};
use vun_trace::tracing::debug;

use crate::{Answer, Ask, Call, Documenter, Documenters, Site, Source, Types};

/// A documenter served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    name: &'static str,
}

impl Remote {
    /// The documenter a component serves, its name asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let reply = component.call::<Documenters>(&Call::Name, &|ask| Self::unasked(&ask))?;
        let name = reply.name().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", documenter = name, library = ?component.path(), "opened a documenter component");
        Ok(Self { component, name })
    }

    /// What a call that documents nothing is answered with.
    fn unasked(ask: &Ask) -> Answer {
        panic!("a documenter asked {ask:?} of a call with nothing to document")
    }
}

impl Documenter for Remote {
    fn name(&self) -> &'static str {
        self.name
    }

    fn document(&self, sources: &[Source], types: &dyn Types) -> Result<Site, Cause> {
        let call = Call::Document(Cow::Borrowed(sources));
        let answer = |ask: Ask| {
            let Ask::IsType { path, name } = ask;
            Answer::IsType(types.is_type(&path, &name))
        };
        let reply = self.component.call::<Documenters>(&call, &answer);
        match reply.and_then(|reply| reply.documented().map_err(Fault::Malformed)) {
            Ok(documented) => documented,
            Err(fault) => Err(Cause::of(&fault)),
        }
    }
}
