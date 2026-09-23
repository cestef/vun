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
