//! The terminal diagnostics renderer as a component of the toolchain (decision 498): linked into
//! `vun`, or built with the `export` feature as a library of its `components` directory.

use std::io::Write;

use vun_diag::{Color, DiagSink};
use vun_renderer::Renderer;

/// The renderer this component serves.
#[must_use]
pub fn component() -> impl Renderer + 'static {
    Terminal
}

struct Terminal;

impl Renderer for Terminal {
    fn name(&self) -> &'static str {
        "terminal"
    }

    fn about(&self) -> &'static str {
        "source excerpts with labels, notes and fixes"
    }

    fn sink<'w>(&self, out: &'w mut dyn Write, color: Color) -> Box<dyn DiagSink + 'w> {
        Box::new(vun_render_annotate::AnnotateRenderer::new(out, color))
    }
}

#[cfg(feature = "export")]
vun_component::export!(vun_renderer::Renderers, component());
