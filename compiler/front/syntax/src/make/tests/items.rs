use vun_grammar::TokenKind;
use vun_items::lang;

use super::read;
use crate::make::{Bind, Expr, Field, File, Item, Member, Param};

fn item(name: &str, value: Expr) -> Item {
    Bind::new(name, value).item()
}

fn field(name: &str, ty: Expr) -> Member {
    Member::Field(Field::new(name, ty))
}

#[test]
fn made_items_print_as_the_parser_reads_them() {
    let u32 = Expr::call(Expr::decl(lang::int::unsigned), vec![Expr::int(32)]);
    let pointer = Expr::prefix(TokenKind::Question, Expr::prefix(TokenKind::Star, Expr::Unit));
    let callback = Expr::Attributed(
        vec![Expr::decl(lang::abi::c)],
        Box::new(Expr::FnType(vec![Param::named("user", pointer.clone())], Some(Box::new(u32.clone())))),
    );
    let file = File {
        doc: vec!["Made.".to_owned()],
        items: vec![
            Item {
                doc: vec!["A point.".to_owned()],
                attrs: vec![Expr::decl(lang::layout::c)],
                ..item(
                    "point",
                    Expr::Struct(vec![
                        field("x", u32.clone()),
                        field(
                            "inner",
                            Expr::Struct(vec![field(
                                "match",
                                Expr::ArrayType(Box::new(Expr::int(4)), Box::new(u32.clone())),
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
                ..item("area", Expr::Hole).typed(Expr::prefix(
                    TokenKind::Unsafe,
                    Expr::FnType(vec![Param::named("p", pointer)], Some(Box::new(u32))),
                ))
            },
            item("counter", Expr::Hole).typed(Expr::decl(lang::int::i32)).mutable(true),
            item("LIMIT", Expr::int(-16)),
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
    read(&text);
}
