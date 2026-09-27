use vun_diag::{ChildKind, Color, DiagSink, Diagnostic, Edit, Fix, Label, Msg, Safety, errors};
use vun_origin::{OriginTable, Resolve};
use vun_span::{SourceFiles, Span};

use crate::AriadneRenderer;

fn render(d: &Diagnostic, origins: &dyn Resolve, files: &SourceFiles) -> String {
    let mut out = Vec::new();
    AriadneRenderer::new(&mut out, Color::Never).emit(d, origins, files);
    String::from_utf8(out).unwrap()
}

#[test]
fn renders_labels_notes_and_fixes() {
    let files = SourceFiles::new();
    let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
    let f = files.add("src/geom.vun", text);
    let span = |needle: &str| {
        let lo = u32::try_from(text.find(needle).unwrap()).unwrap();
        Span::new(f, lo, lo + u32::try_from(needle.len()).unwrap())
    };
    let origins = OriginTable::new();
    let (sum, result) = (origins.source(span("p.x.abs() + p.y.abs()")), origins.source(span("u32")));
    let mut d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "value does not fit the result type")
        .at(sum, Some(Msg::new().text("this is ").code("Int(0..=4294967296)")))
        .child(ChildKind::Note, "the sum reaches 2^32");
    d.labels.push(Label { origin: result, msg: Some("result type".into()) });
    d.fixes.push(Fix {
        msg: "widen the result type".into(),
        safety: Safety::Safe,
        edits: vec![Edit { span: span("u32"), text: "u64".into(), holes: Vec::new() }],
        preferred: true,
    });
    let out = render(&d, &origins, &files);
    for expected in [
        "[int.does_not_fit] Error: value does not fit the result type",
        "src/geom.vun:2:5",
        "this is `Int(0..=4294967296)`",
        "result type",
        "Note: the sum reaches 2^32",
        "Help: widen the result type [safe], at src/geom.vun:1:24: `u32` becomes `u64`",
    ] {
        assert!(out.contains(expected), "missing {expected:?} in\n{out}");
    }
}

#[test]
fn renders_without_a_location() {
    let d = Diagnostic::error(errors::IO_UNREADABLE_FILE, "cannot read `x.vun`");
    let out = render(&d, &OriginTable::new(), &SourceFiles::new());
    assert!(out.starts_with("error[io.unreadable_file]: cannot read `x.vun`"), "{out}");
}
