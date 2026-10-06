use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vun_component::Component;
use vun_diag::{Color, DiagSink, Diagnostic, errors};
use vun_origin::{OriginTable, Resolve};
use vun_render_short::ShortRenderer;
use vun_span::{SourceFiles, SourceMap, Span};

use crate::{Remote, Renderer, Renderers};

/// Shows each diagnostic as the short renderer does.
struct Plain;

impl Renderer for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }

    fn about(&self) -> &'static str {
        "one line each"
    }

    fn sink<'w>(&self, out: &'w mut dyn Write, color: Color) -> Box<dyn DiagSink + 'w> {
        Box::new(ShortRenderer::new(out, color))
    }
}

#[test]
fn a_diagnostic_is_shown_through_the_boundary_as_it_is_linked() {
    let remote =
        Remote::of(Component::within(vun_component::descriptor!(Renderers, Plain)).unwrap()).unwrap();
    assert_eq!((remote.name(), remote.about()), ("plain", "one line each"));
    // The tables are the process's, as the library reads them.
    let (files, origins) = (SourceFiles::global(), OriginTable::global());
    let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
    let file = files.add("src/geom.vun", text);
    let expr = "p.x.abs() + p.y.abs()";
    let lo = u32::try_from(text.find(expr).unwrap()).unwrap();
    let at = origins.source(Span::new(file, lo, lo + u32::try_from(expr.len()).unwrap()));
    let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "value does not fit the result type").at(at, None);
    let shown = |renderer: &dyn Renderer| {
        let mut out = Vec::new();
        renderer.sink(&mut out, Color::Never).emit(&d, origins, files);
        out
    };
    let linked = shown(&Plain);
    assert!(!linked.is_empty());
    assert_eq!(shown(&remote), linked);
}

/// Keeps how many diagnostics it was shown until the run is over, then writes it.
#[derive(Default)]
struct Tally(Arc<AtomicUsize>);

struct Counted(Arc<AtomicUsize>);

impl DiagSink for Counted {
    fn emit(&mut self, _: &Diagnostic, _: &dyn Resolve, _: &dyn SourceMap) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

impl Renderer for Tally {
    fn name(&self) -> &'static str {
        "tally"
    }

    fn about(&self) -> &'static str {
        "how many, at the end"
    }

    fn sink<'w>(&self, _: &'w mut dyn Write, _: Color) -> Box<dyn DiagSink + 'w> {
        Box::new(Counted(self.0.clone()))
    }

    fn finish(&self, out: &mut dyn Write) {
        write!(out, "{}", self.0.load(Ordering::Relaxed)).unwrap();
    }
}

#[test]
fn what_a_renderer_kept_of_the_run_is_written_once_through_the_boundary() {
    let component = Component::within(vun_component::descriptor!(Renderers, Tally::default())).unwrap();
    let remote = Remote::of(component).unwrap();
    let (files, origins) = (SourceFiles::global(), OriginTable::global());
    let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "wide");
    for _ in 0..3 {
        remote.sink(&mut Vec::new(), Color::Never).emit(&d, origins, files);
    }
    let mut out = Vec::new();
    remote.finish(&mut out);
    assert_eq!(out, b"3");
}
