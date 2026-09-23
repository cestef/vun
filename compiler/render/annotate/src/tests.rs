use overt_diag::{ChildKind, Color, DiagSink, Diagnostic, Edit, Fix, Label, Msg, Safety, errors};
use overt_origin::OriginTable;
use overt_span::{SourceFiles, Span};

use crate::AnnotateRenderer;

fn render(d: &Diagnostic, origins: &OriginTable, files: &SourceFiles) -> String {
    let mut out = Vec::new();
    AnnotateRenderer::new(&mut out, Color::Never).emit(d, origins, files);
    String::from_utf8(out).unwrap()
}

#[test]
fn renders_labels_notes_and_fixes() {
    let mut files = SourceFiles::new();
    let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
    let f = files.add("src/geom.ov", text);
    let span = |needle: &str| {
        let lo = u32::try_from(text.find(needle).unwrap()).unwrap();
        Span::new(f, lo, lo + u32::try_from(needle.len()).unwrap())
    };
    let mut origins = OriginTable::new();
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
        "error[int.does_not_fit]: value does not fit the result type",
        "src/geom.ov:2:5",
        "this is `Int(0..=4294967296)`",
        "result type",
        "note: the sum reaches 2^32",
        "help: widen the result type [safe]",
        "norm1 := (p: Point) -> u64 {",
    ] {
        assert!(out.contains(expected), "missing {expected:?} in\n{out}");
    }
}

#[test]
fn renders_without_a_location() {
    let d = Diagnostic::error(errors::IO_UNREADABLE_FILE, "cannot read `x.ov`");
    let out = render(&d, &OriginTable::new(), &SourceFiles::new());
    assert!(out.starts_with("error[io.unreadable_file]: cannot read `x.ov`"), "{out}");
}
