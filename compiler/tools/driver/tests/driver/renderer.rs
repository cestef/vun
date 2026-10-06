//! Renderers as components (decision 498): the terminal renderer loaded from its library shows a
//! diagnostic as the linked one does, from origins and files it reads as its own.

use vun_component_renderer_terminal::component;
use vun_diag::{ChildKind, Color, Diagnostic, errors};
use vun_driver::{Components, Provider, Registry};
use vun_origin::OriginTable;
use vun_renderer::Renderer;
use vun_span::{SourceFiles, Span};

use crate::common::built_component;

const CRATE: &str = "vun-component-renderer-terminal";

#[test]
fn a_renderer_loaded_from_a_library_shows_a_diagnostic_as_the_linked_one_does() {
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component(CRATE)));
    let loaded = registry.renderers.get("terminal").expect("the library serves a renderer");
    assert!(matches!(loaded.from, Provider::Loaded(_)));

    let (files, origins) = (SourceFiles::global(), OriginTable::global());
    let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
    let file = files.add("src/geom.vun", text);
    let expr = "p.x.abs() + p.y.abs()";
    let lo = u32::try_from(text.find(expr).unwrap()).unwrap();
    let at = origins.source(Span::new(file, lo, lo + u32::try_from(expr.len()).unwrap()));
    let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "value does not fit the result type")
        .at(at, None)
        .child(ChildKind::Note, "the sum reaches 2^32");
    let shown = |renderer: &dyn Renderer| {
        let mut out = Vec::new();
        renderer.sink(&mut out, Color::Never).emit(&d, origins, files);
        String::from_utf8(out).unwrap()
    };
    let linked = shown(&component());
    assert!(linked.contains("value does not fit the result type"), "{linked}");
    assert_eq!(shown(&*loaded.item), linked);
}
