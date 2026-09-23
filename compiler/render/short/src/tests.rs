use super::*;
use overt_diag::errors;
use overt_span::{SourceFiles, Span};

#[test]
fn renders_location_and_relative_id() {
    let files = SourceFiles::new();
    let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
    let f = files.add("src/geom.ov", text);
    let expr = "p.x.abs() + p.y.abs()";
    let lo = u32::try_from(text.find(expr).unwrap()).unwrap();
    let origins = OriginTable::new();
    let at = origins.source(Span::new(f, lo, lo + u32::try_from(expr.len()).unwrap()));
    let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "value does not fit the result type").at(at, None);

    let mut out = Vec::new();
    ShortRenderer::new(&mut out).emit(&d, &origins, &files);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "src/geom.ov:2:5: error[int.does_not_fit]: value does not fit the result type\n"
    );
}
