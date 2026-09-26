use super::*;
use vun_diag::{ChildKind, Msg, errors};
use vun_span::{SourceFiles, Span};

#[test]
fn a_diagnostic_is_one_line_with_resolved_locations_and_typed_parts() {
    let files = SourceFiles::new();
    let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
    let f = files.add("src/geom.vun", text);
    let expr = "p.x.abs() + p.y.abs()";
    let lo = u32::try_from(text.find(expr).unwrap()).unwrap();
    let origins = OriginTable::new();
    let at = origins.source(Span::new(f, lo, lo + u32::try_from(expr.len()).unwrap()));
    let msg = Msg::new().text("value does not fit ").ty("u32");
    let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, msg).at(at, None).child(ChildKind::Help, "widen it");

    let mut out = Vec::new();
    JsonRenderer::new(&mut out).emit(&d, &origins, &files);
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1);
    let v = vun_json::parse(&text).unwrap();
    assert_eq!((v["type"].as_str(), v["schema"].as_u64()), (Some("diagnostic"), Some(SCHEMA)));
    assert_eq!((v["id"].as_str(), v["severity"].as_str()), (Some("int.does_not_fit"), Some("error")));
    let start = &v["primary"]["at"]["start"];
    assert_eq!(
        (v["primary"]["at"]["file"].as_str(), start["line"].as_u64(), start["column"].as_u64()),
        (Some("src/geom.vun"), Some(2), Some(5))
    );
    assert_eq!(v["message"]["parts"][1]["kind"].as_str(), Some("type"));
    assert_eq!(v["children"][0]["kind"].as_str(), Some("help"));
}
