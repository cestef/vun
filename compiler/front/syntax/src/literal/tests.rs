use super::*;
use crate::cst::Cst;
use crate::tokens::Tokens;
use vun_diag::{DiagCtxt, DiagCx};
use vun_origin::OriginTable;
use vun_span::SourceFiles;

fn read(text: &str) -> Vec<Decl<'_>> {
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    assert_eq!(diag.error_count(), 0);
    cst.declarations(&tokens, text)
}

#[test]
fn reads_literals_and_leaves_the_rest_opaque() {
    let text = "/// doc\nxs := [.a, .\\unsafe, \"s\", -0x1_0];\nr: T = .{ .k = 1, .l = .{ 2 } };\n\
                E := enum { /// v\n a, b(u8) };\nS := struct { f: u8, d := f() + 1; };\n";
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
    let names: Vec<_> = members.iter().map(|m| (m.name, &m.value)).collect();
    assert_eq!(names, [("f", &Literal::Opaque), ("d", &Literal::Opaque)]);
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
        // A comma in a comment is no field's.
        ("p := .{\n  .a = 1 // x, y\n};", "p := .{\n  .a = 1,\n  .hal = .{ .path = \"../hal\" } // x, y\n};"),
    ];
    for (before, after) in cases {
        let edit = read(before)[0].insert(before, "hal", &dep).unwrap();
        assert_eq!(vun_diag::Edit::apply(&[edit], before).unwrap(), after);
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
    let after = vun_diag::Edit::apply(&[deps.insert(before, "hal", &dep).unwrap()], before).unwrap();
    let expected = format!(
        "package := .{{\n    .name = \"app\",\n    .deps = .{{\n        .util = .{{ .path = \"../util\" }},\n        \
         .hal = .{{\n            .version = \"1.0.0\",\n            .urls = .{{\n                \"{long}\",\n            \
         }},\n        }},\n    }},\n}};"
    );
    assert_eq!(after, expected);
    let read_back = read(&after)[0].value.decls().unwrap()[1].value.get("hal").unwrap().source();
    assert_eq!(read_back, dep.source());
}

#[test]
fn typed_literals_and_path_calls_read_and_write_back() {
    let text = "d := diag.Diag{ .explanation = meta.embed(\"errors/a.md\") };";
    let value = &read(text)[0].value;
    let Literal::Typed(ty, _) = value else { panic!("{value:?}") };
    assert_eq!(ty, &["diag", "Diag"]);
    let (path, args) = value.get("explanation").and_then(Literal::as_call).unwrap();
    assert_eq!((path, args), (&["meta", "embed"][..], &[Literal::String("errors/a.md".into())][..]));
    assert_eq!(value.source().unwrap(), "diag.Diag{ .explanation = meta.embed(\"errors/a.md\") }");
}

#[test]
fn values_are_set_in_place() {
    let before = "p := .{\n    .hal = .{ .version = \"1.0.0\" },\n};";
    let hal = read(before)[0].value.decls().unwrap()[0].clone();
    let newer = read("v := .{ .version = \"1.2.0\" };")[0].value.clone();
    let after = vun_diag::Edit::apply(&[hal.set(before, &newer).unwrap()], before).unwrap();
    assert_eq!(after, "p := .{\n    .hal = .{ .version = \"1.2.0\" },\n};");
}

#[test]
fn reads_and_writes_holes() {
    let decls = read("Int: ($r: Range) -> type = _;\nx := f(_);");
    assert_eq!((decls[0].name, &decls[0].value), ("Int", &Literal::Hole));
    assert_eq!(Literal::Hole.source().as_deref(), Ok("_"));
}

#[test]
fn paths_read_and_write_back() {
    let text = "lints := .{ .{ std.lints.pedantic, .warn } };";
    let value = &read(text)[0].value;
    let Literal::List(items) = value else { panic!("{value:?}") };
    let Literal::List(pair) = &items[0] else { panic!("{items:?}") };
    assert_eq!(pair[0], Literal::Path(vec!["std", "lints", "pedantic"]));
    assert_eq!(value.source().unwrap(), ".{ .{ std.lints.pedantic, .warn } }");
}

#[test]
fn integers_are_exact_in_every_radix() {
    let big = num_bigint::BigInt::from(1) << 128u32;
    assert_eq!(integer("0x1_0000_0000_0000_0000_0000_0000_0000_0000"), Some(big.clone()));
    assert_eq!(integer("340_282_366_920_938_463_463_374_607_431_768_211_456"), Some(big));
    assert_eq!(integer("0b1010"), Some(10.into()));
    assert_eq!(integer("0o17"), Some(15.into()));
    assert_eq!(integer("0"), Some(0.into()));
    assert_eq!(integer("0x"), None);
}

#[derive(Debug, PartialEq, Eq, FromLiteral)]
enum Term {
    Many(#[literal(rest)] Vec<Term>),
    Of(Name, Path),
    #[literal(name = "eof")]
    End,
    #[literal(untagged)]
    Text(String),
    #[literal(untagged)]
    Field(Name),
}

#[derive(Debug, PartialEq, Eq, FromLiteral)]
struct Rules {
    terms: Vec<Term>,
    #[literal(name = "Levels")]
    levels: Fields<Vec<u32>>,
    absent: Option<Name>,
}

#[test]
fn typed_values_read_from_literals() {
    let decls = read("terms := [.many(\"a\", .b), .of(.c, d.e), .eof];\nLevels := .{ .x = [1, 2] };");
    let rules = Rules::from_decls(&decls).unwrap();
    let name = |n: &str| Name(n.to_owned());
    assert_eq!(
        rules.terms,
        [
            Term::Many(vec![Term::Text("a".to_owned()), Term::Field(name("b"))]),
            Term::Of(name("c"), Path(vec!["d".to_owned(), "e".to_owned()])),
            Term::End,
        ]
    );
    assert_eq!(rules.levels, Fields(vec![("x".to_owned(), vec![1, 2])]));
    assert_eq!(rules.absent, None);
}

#[test]
fn mismatches_name_where_and_what() {
    let decls = read("terms := [.of(.c)];\nLevels := .{};");
    let mismatch = Rules::from_decls(&decls).unwrap_err();
    assert_eq!(mismatch.expected, Expected::Arguments(2));
    assert_eq!(mismatch.path().collect::<Vec<_>>(), [Step::Field("terms"), Step::Item(0)]);
    let decls = read("terms := [];");
    let mismatch = Rules::from_decls(&decls).unwrap_err();
    assert_eq!(
        (mismatch.expected, mismatch.path().collect::<Vec<_>>()),
        (Expected::Field, vec![Step::Field("Levels")])
    );
}

#[test]
fn fields_keep_their_docs() {
    let decls = read("r := .{\n    /// what x is\n    .x = 1,\n    .y = 2,\n};");
    let Literal::Record(fields) = &decls[0].value else { panic!("{:?}", decls[0].value) };
    assert_eq!((fields[0].name, fields[0].doc.as_slice()), ("x", ["what x is"].as_slice()));
    assert_eq!(fields[1].doc, Vec::<&str>::new());
}
