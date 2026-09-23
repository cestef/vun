use super::*;
use crate::{lexer::lex, parser::parse};
use overt_diag::{DiagCtxt, DiagCx};
use overt_origin::OriginTable;
use overt_span::SourceFiles;

fn read(text: &str) -> Vec<Decl<'_>> {
    let file = SourceFiles::new().add("t.ov", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
    let tokens = lex(file, text, &mut cx);
    let cst = parse(&tokens, text, &mut cx);
    assert_eq!(diag.error_count(), 0);
    declarations(&cst, &tokens, text)
}

#[test]
fn reads_literals_and_skips_the_rest() {
    let text = "/// doc\nxs := [.a, .\\unsafe, \"s\", -0x1_0];\nr: T = .{ .k = 1, .l = .{ 2 } };\n\
                E := enum { /// v\n a, b(u8) };\nS := struct { f: u8, d := f(); };\n";
    let decls = read(text);
    let names: Vec<_> = decls.iter().map(|d| d.name).collect();
    assert_eq!(names, ["xs", "r", "E", "S"]);
    assert_eq!(decls[0].doc, ["doc"]);
    assert_eq!(
        decls[0].value,
        Literal::List(vec![
            Literal::Name("a"),
            Literal::Name("unsafe"),
            Literal::String("s".into()),
            Literal::Int(-16)
        ])
    );
    assert_eq!(decls[1].value.get("l"), Some(&Literal::List(vec![Literal::Int(2)])));
    let variants = decls[2].value.decls().unwrap();
    assert_eq!((variants[0].name, &variants[0].doc[..], variants[1].name), ("a", &["v"][..], "b"));
    let members = decls[3].value.decls().unwrap();
    assert_eq!((members.len(), members[0].name, &members[0].value), (1, "d", &Literal::Opaque));
}

#[test]
fn reads_tagged_values() {
    let decls = read("t := [.one(.a), .pair(.b, [1]), .none()];");
    assert_eq!(
        decls[0].value,
        Literal::List(vec![
            Literal::Tagged("one", vec![Literal::Name("a")]),
            Literal::Tagged("pair", vec![Literal::Name("b"), Literal::List(vec![Literal::Int(1)])]),
            Literal::Tagged("none", vec![]),
        ])
    );
    assert_eq!(decls[0].value.as_list().unwrap()[0].as_tagged(), Some(("one", &[Literal::Name("a")][..])));
}

#[test]
fn written_literals_read_back() {
    let text = "v := .{ .name = \"a \\\"q\\\"\\n\", .n = -3, .xs = .{ .a, .\\if, .t(1, .{}) }, .e = .{} };";
    let value = &read(text)[0].value;
    let written = format!("v := {};", value.source().unwrap());
    assert_eq!(&read(&written)[0].value, value);
    assert_eq!(Literal::Opaque.source(), Err(Unwritable::Opaque));
    assert_eq!(Literal::Name("not a name").source(), Err(Unwritable::Name));
}

#[test]
fn fields_are_inserted_after_the_last_one() {
    let dep = read("d := .{ .path = \"../hal\" };")[0].value.clone();
    let cases = [
        ("p := .{};", "p := .{ .hal = .{ .path = \"../hal\" } };"),
        ("p := .{ .a = 1 };", "p := .{ .a = 1, .hal = .{ .path = \"../hal\" } };"),
        ("p := .{ .a = 1, };", "p := .{ .a = 1, .hal = .{ .path = \"../hal\" }, };"),
        ("p := .{\n    .a = 1,\n};", "p := .{\n    .a = 1,\n    .hal = .{ .path = \"../hal\" },\n};"),
        ("p := .{\n  .a = 1\n};", "p := .{\n  .a = 1,\n  .hal = .{ .path = \"../hal\" }\n};"),
    ];
    for (before, after) in cases {
        let edit = read(before)[0].insert(before, "hal", &dep).unwrap();
        assert_eq!(overt_diag::Edit::apply(&[edit], before), after);
    }
    assert_eq!(read("p := 1;")[0].insert("p := 1;", "a", &dep), Err(Unwritable::NotRecord));
}

#[test]
fn long_insertions_are_laid_out_one_field_per_line() {
    let long = "x".repeat(WIDTH);
    let entry = format!("d := .{{ .version = \"1.0.0\", .urls = .{{ \"{long}\" }} }};");
    let dep = read(&entry)[0].value.clone();
    let before =
        "package := .{\n    .name = \"app\",\n    .deps = .{ .util = .{ .path = \"../util\" } },\n};";
    let deps = read(before)[0].value.decls().unwrap()[1].clone();
    let after = overt_diag::Edit::apply(&[deps.insert(before, "hal", &dep).unwrap()], before);
    let expected = format!(
        "package := .{{\n    .name = \"app\",\n    .deps = .{{\n        .util = .{{ .path = \"../util\" }},\n        \
         .hal = .{{\n            .version = \"1.0.0\",\n            .urls = .{{\n                \"{long}\",\n            \
         }},\n        }},\n    }},\n}};"
    );
    assert_eq!(after, expected);
    let read_back = read(&after)[0].value.decls().unwrap()[1].value.get("hal").unwrap().source();
    assert_eq!(read_back, dep.source());
}
