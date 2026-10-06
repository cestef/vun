//! The host's side: a formatter that is a component, called through the boundary.

use std::borrow::Cow;

use vun_component::{Cause, Component, Fault};
use vun_trace::tracing::debug;

use crate::{Call, Formatter, Formatters, Unlaid};

/// A formatter served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    name: &'static str,
}

impl Remote {
    /// The formatter a component serves, its name asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let name = component.call::<Formatters>(&Call::Name, &|()| ())?.name().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", formatter = name, library = ?component.path(), "opened a formatter component");
        Ok(Self { component, name })
    }
}

impl Formatter for Remote {
    fn name(&self) -> &'static str {
        self.name
    }

    fn layout(&self, text: &str, width: usize) -> Result<String, Unlaid> {
        let call =
            Call::Layout { text: Cow::Borrowed(text), width: u64::try_from(width).unwrap_or(u64::MAX) };
        let reply = self.component.call::<Formatters>(&call, &|()| ());
        match reply.and_then(|reply| reply.laid().map_err(Fault::Malformed)) {
            Ok(laid) => laid,
            Err(fault) => Err(Unlaid::Faulted(Cause::of(&fault))),
        }
    }
}
