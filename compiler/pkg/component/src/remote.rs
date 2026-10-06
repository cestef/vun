//! The host's side: a scheme that is a component, called through the boundary.

use vun_component::{Component, Fault};
use vun_fetch::{Failure, Scheme};
use vun_trace::tracing::debug;
use vun_uri::Uri;

use crate::{Call, Schemes};

/// A scheme served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    name: &'static str,
}

impl Remote {
    /// The scheme a component serves, its name asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let name = component.call::<Schemes>(&Call::Name, &|()| ())?.name().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", scheme = name, library = ?component.path(), "opened a scheme component");
        Ok(Self { component, name })
    }
}

impl Scheme for Remote {
    fn name(&self) -> &str {
        self.name
    }

    fn read(&self, uri: &Uri) -> Result<Vec<u8>, Failure> {
        let reply = self.component.call::<Schemes>(&Call::Read(uri.clone()), &|()| ());
        let read = reply
            .and_then(|reply| reply.read().map_err(Fault::Malformed))
            .map_err(|fault| Failure::Download(Box::new(fault)))?;
        read.map_err(|cause| Failure::Download(Box::new(cause)))
    }
}
