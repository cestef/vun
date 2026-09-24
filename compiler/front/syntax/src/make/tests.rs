use vun_diag::{DiagCtxt, DiagCx};
use vun_grammar::TokenKind;
use vun_items::lang;
use vun_origin::OriginTable;
use vun_span::SourceFiles;

use super::{Expr, Field, File, Item};
use crate::cst::Cst;
use crate::tokens::Tokens;

/// Whether `text` lexes and parses without a report.
fn parses(text: &str) -> bool {
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    Cst::parse(&tokens, text, &mut cx);
    diag.error_count() == 0
}

fn item(name: &str, ty: Option<Expr>, value: Expr) -> Item {
    Item { doc: Vec::new(), attrs: Vec::new(), mutable: false, name: name.to_owned(), ty, value }
}

fn field(name: &str, ty: Expr) -> Field {
    Field { name: name.to_owned(), ty }
}

#[test]
fn made_items_print_as_the_parser_reads_them() {
    let u32 = Expr::call(Expr::decl(lang::int::unsigned), vec![Expr::Int(32)]);
    let pointer = Expr::prefix(TokenKind::Question, Expr::prefix(TokenKind::Star, Expr::Unit));
    let callback = Expr::Attributed(
        vec![Expr::decl(lang::abi::c)],
        Box::new(Expr::Fn(vec![field("user", pointer.clone())], Some(Box::new(u32.clone())))),
    );
    let file = File {
        doc: vec!["Made.".to_owned()],
        items: vec![
            Item {
                doc: vec!["A point.".to_owned()],
                attrs: vec![Expr::decl(lang::layout::c)],
                ..item(
                    "point",
                    None,
                    Expr::Struct(vec![
                        field("x", u32.clone()),
                        field(
                            "inner",
                            Expr::Struct(vec![field(
                                "match",
                                Expr::Array(Box::new(Expr::Int(4)), Box::new(u32.clone())),
                            )]),
                        ),
                        field("on_done", Expr::prefix(TokenKind::Question, callback)),
                    ]),
                )
            },
            Item {
                attrs: vec![
                    Expr::call(Expr::decl(lang::link::symbol), vec![Expr::String("area".to_owned())]),
                    Expr::decl(lang::abi::c),
                ],
                ..item(
                    "area",
                    Some(Expr::prefix(
                        TokenKind::Unsafe,
                        Expr::Fn(vec![field("p", pointer)], Some(Box::new(u32))),
                    )),
                    Expr::Hole,
                )
            },
            Item { mutable: true, ..item("counter", Some(Expr::decl(lang::int::i32)), Expr::Hole) },
            item("LIMIT", None, Expr::Int(-16)),
        ],
    };
    let text = file.source();
    assert_eq!(
        text,
        "//! Made.

/// A point.
#[lang.layout.c]
point := struct {
    x: lang.int.unsigned(32),
    inner: struct {
        \\match: [4]lang.int.unsigned(32),
    },
    on_done: ?#[lang.abi.c] (user: ?*()) -> lang.int.unsigned(32),
};

#[lang.link.symbol(\"area\"), lang.abi.c]
area: unsafe (p: ?*()) -> lang.int.unsigned(32) = _;

mut counter: lang.int.i32 = _;

LIMIT := -16;
"
    );
    assert!(parses(&text), "{text}");
}
