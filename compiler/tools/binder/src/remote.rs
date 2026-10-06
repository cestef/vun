//! The host's side: a binder that is a component, called through the boundary.

use vun_component::{Component, Fault};
use vun_trace::tracing::debug;

use crate::{Binder, Binders, Bindings, Call, Failure, Request};

/// A binder served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    name: &'static str,
}

impl Remote {
    /// The binder a component serves, its name asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let name = component.call::<Binders>(&Call::Name, &|()| ())?.name().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", binder = name, library = ?component.path(), "opened a binder component");
        Ok(Self { component, name })
    }
}

impl Binder for Remote {
    fn name(&self) -> &'static str {
        self.name
    }

    /// A component that cannot be reached is as unreadable as the header would be.
    fn bind(&self, request: &Request) -> Result<Bindings, Failure> {
        let reply = self.component.call::<Binders>(&Call::Bind(request.clone()), &|()| ());
        match reply.and_then(|reply| reply.bound().map_err(Fault::Malformed)) {
            Ok(bound) => bound,
            Err(fault) => Err(Failure::Errors(vec![fault.to_string()])),
        }
    }
}
