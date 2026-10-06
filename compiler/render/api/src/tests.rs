use std::io::Write;

use vun_component::Component;
use vun_diag::{Color, DiagSink, Diagnostic, errors};
use vun_origin::OriginTable;
use vun_render_short::ShortRenderer;
use vun_span::{SourceFiles, Span};

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
