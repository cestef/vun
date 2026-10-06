//! The host's side: a server that is a component, called through the boundary.

use std::io;

use vun_command::Outcome;
use vun_component::{Component, Fault};
use vun_diag::DiagCtxt;
use vun_trace::tracing::debug;

use crate::{Call, Server, Servers};

/// A server served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    name: &'static str,
}

impl Remote {
    /// The server a component serves, its name asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let name = component.call::<Servers>(&Call::Name, &|()| ())?.name().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", server = name, library = ?component.path(), "opened a server component");
        Ok(Self { component, name })
    }
}

impl Server for Remote {
    fn name(&self) -> &'static str {
        self.name
    }

    fn serve(&self, diag: &mut DiagCtxt) -> io::Result<Outcome> {
        let reply = self.component.call::<Servers>(&Call::Serve, &|()| ());
        let served = reply
            .and_then(|reply| reply.served().map_err(Fault::Malformed))
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
        let (outcome, theirs) = served;
        diag.absorb(&theirs);
        Ok(outcome)
    }
}
