//! The SARIF diagnostics renderer as a component of the toolchain (decision 498): linked into
//! `vun`, or built with the `export` feature as a library of its `components` directory. It keeps
//! every diagnostic of the run and writes the one log when the run is over.

use std::io::Write;

use vun_diag::{Color, DiagSink};
use vun_render_sarif::Sarif;
use vun_renderer::Renderer;

/// The renderer this component serves.
#[must_use]
pub fn component() -> impl Renderer + 'static {
    Log::default()
}

#[derive(Default)]
struct Log(Sarif);

impl Renderer for Log {
    fn name(&self) -> &'static str {
        "sarif"
    }

    fn about(&self) -> &'static str {
        "one SARIF 2.1.0 log of the run, on standard output"
    }

    fn sink<'w>(&self, _: &'w mut dyn Write, _: Color) -> Box<dyn DiagSink + 'w> {
        Box::new(self.0.clone())
    }

    fn finish(&self, out: &mut dyn Write) {
        // Best-effort output: a closed stream must not abort compilation.
        let _ = self.0.write(out);
    }
}

#[cfg(feature = "export")]
vun_component::export!(vun_renderer::Renderers, component());
