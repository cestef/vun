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

#[test]
fn a_renderer_that_keeps_the_run_writes_one_log_from_its_library() {
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component("vun-component-renderer-sarif")));
    let loaded = registry.renderers.get("sarif").expect("the library serves a renderer");

    let (files, origins) = (SourceFiles::global(), OriginTable::global());
    let file = files.add("src/answer.vun", "answer: u32 = 1;\n");
    let at = origins.source(Span::new(file, 0, 6));
    let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "value does not fit").at(at, None);
    for _ in 0..2 {
        loaded.item.sink(&mut Vec::new(), Color::Never).emit(&d, origins, files);
    }
    let mut out = Vec::new();
    loaded.item.finish(&mut out);
    let log = vun_data::json::parse(std::str::from_utf8(&out).unwrap()).unwrap();
    let run = &log["runs"][0];
    assert_eq!(run["results"].as_array().map(<[_]>::len), Some(2));
    assert_eq!(run["results"][1]["ruleId"].as_str(), Some("int.does_not_fit"));
    let region = &run["results"][1]["locations"][0]["physicalLocation"]["region"];
    assert_eq!((region["startColumn"].as_u64(), region["endColumn"].as_u64()), (Some(1), Some(7)));
}
