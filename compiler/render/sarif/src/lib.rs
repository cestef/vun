//! Diagnostics as SARIF 2.1.0: one log for a run, so a sink adds each diagnostic to it and
//! [`Sarif::write`] writes it once the run is over.

mod log;
mod place;
mod result;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::sync::{Arc, Mutex, PoisonError};

use vun_data::json::Json;
use vun_diag::{DiagSink, Diagnostic};
use vun_origin::Resolve;
use vun_span::SourceMap;

pub use log::{Log, SCHEMA, VERSION};

/// The log of a run, shared by every sink that adds to it.
#[derive(Clone, Default)]
pub struct Sarif {
    log: Arc<Mutex<Log>>,
}

impl Sarif {
    /// Writes the log, one line.
    ///
    /// # Errors
    /// Returns why `out` could not be written.
    pub fn write(&self, out: &mut dyn Write) -> io::Result<()> {
        let document = self.log.lock().unwrap_or_else(PoisonError::into_inner).document();
        writeln!(out, "{}", Json(&document))
    }
}

impl DiagSink for Sarif {
    fn emit(&mut self, d: &Diagnostic, origins: &dyn Resolve, sources: &dyn SourceMap) {
        self.log.lock().unwrap_or_else(PoisonError::into_inner).add(d, origins, sources);
    }
}
