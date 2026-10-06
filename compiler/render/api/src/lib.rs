//! Renderers as components (decision 498): a [`Renderer`] shows diagnostics for people (spec 10,
//! section 2), as `--diagnostics` chooses. [`Renderers`] is the interface a renderer library is
//! served through, [`Remote`] the `Renderer` the host shows diagnostics by. The renderer is the
//! same crate linked in or loaded. The origins and files of a diagnostic cross by content, so the
//! library reads them as its own; it asks the host only where a file was read again.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use std::io::Write;

use vun_diag::{Color, DiagSink};

pub use protocol::{Answer, Ask, Call, Info, Reply, Show};
pub use remote::Remote;
pub use serve::Renderers;

pub trait Renderer: Send + Sync {
    /// What `--diagnostics` calls it: `terminal`.
    fn name(&self) -> &'static str;

    /// What it shows, for the help of `--diagnostics`.
    fn about(&self) -> &'static str;

    /// A sink writing to `out`, in `color`.
    fn sink<'w>(&self, out: &'w mut dyn Write, color: Color) -> Box<dyn DiagSink + 'w>;
}

/// One line per diagnostic, always linked: what shows diagnostics when the one asked for is not
/// there.
pub struct Short;

impl Renderer for Short {
    fn name(&self) -> &'static str {
        "short"
    }

    fn about(&self) -> &'static str {
        "one line per diagnostic: file:line:col: severity[id]: message"
    }

    fn sink<'w>(&self, out: &'w mut dyn Write, color: Color) -> Box<dyn DiagSink + 'w> {
        Box::new(vun_render_short::ShortRenderer::new(out, color))
    }
}
