//! Every form prints as the parser reads it back.

use vun_grammar::{BorrowWord, NodeKind, TokenKind};

use super::{assert_reads_back, read};
use crate::make::{Arm, Bind, Block, Borrow, Expr, Field, File, Function, Init, Member, Param, Variant};

fn name(n: &str) -> Expr {
    Expr::local(n)
}

fn int(n: i128) -> Expr {
    Expr::int(n)
}

fn block(stmts: Vec<Expr>, tail: Option<Expr>) -> Block {
    Block::new(stmts, tail)
}

fn arm(pattern: Expr, value: Expr) -> Arm {
    Arm { pattern, guard: None, value: Some(value) }
}

#[test]
fn literals_and_names() {
    for expr in [
        int(0),
        int(-7),
        Expr::float(1.5),
        Expr::float(-2.0),
        Expr::float(1e21),
        Expr::float(1e-7),
        Expr::float(100_000.0),
        Expr::Char('a'),
        Expr::Char('\''),
        Expr::Char('\n'),
        Expr::Char('é'),
        Expr::String("a \"b\"\n\\".to_owned()),
        Expr::Unit,
        Expr::Path(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]),
        name("match"),
        Expr::DotName("some".to_owned()),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn aggregates() {
    for expr in [
        Expr::Tuple(vec![int(1), name("x")]),
        Expr::Tuple(vec![int(1)]),
        Expr::List(vec![]),
        Expr::List(vec![int(1), int(2)]),
        Expr::Repeat(Box::new(int(0)), Box::new(int(16))),
        Expr::Record(vec![]),
        Expr::Record(vec![Init::Named("x".to_owned(), int(1)), Init::Named("y".to_owned(), name("z"))]),
        Expr::Record(vec![Init::Item(int(1)), Init::Item(int(2))]),
        Expr::Typed(Box::new(name("Point")), vec![Init::Named("x".to_owned(), int(1))]),
        Expr::Typed(Box::new(Expr::call(name("Pair"), vec![name("u8")])), vec![]),
        Expr::ArrayType(Box::new(int(4)), Box::new(Expr::Tuple(vec![name("u8"), name("bool")]))),
        Expr::ArrayType(Box::new(int(2)), Box::new(Expr::ArrayType(Box::new(int(3)), Box::new(name("u8"))))),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn types_and_functions() {
    let sum = Function {
        params: vec![Param::named("a", name("u32")), Param::named("b", name("u32"))],
        result: Some(Box::new(name("u64"))),
        body: block(vec![], Some(Expr::infix(TokenKind::Plus, name("a"), name("b")))),
    };
    let rest = Param { pattern: Some(Expr::prefix(TokenKind::DotDot, name("rest"))), ty: name("P") };
    let generic = Param { pattern: Some(Expr::prefix(TokenKind::Dollar, name("T"))), ty: name("type") };
    for expr in [
        Expr::Function(sum.clone()),
        Expr::Function(Function { params: vec![], result: None, body: Block::default() }),
        Expr::Function(Function { params: vec![generic, rest], ..sum.clone() }),
        Expr::Closure(vec![Expr::borrow(Borrow::MUT, name("n"))], sum),
        Expr::FnType(
            vec![Param::unnamed(name("u8")), Param::unnamed(name("u8"))],
            Some(Box::new(name("u16"))),
        ),
        Expr::FnType(vec![Param::named("x", name("u8"))], None),
        Expr::Struct(vec![]),
        Expr::Struct(vec![
            Member::Field(Field { default: Some(int(0)), ..Field::new("x", name("u8")) }),
            Member::Item(Bind::new("origin", int(1)).item()),
        ]),
        Expr::Enum(vec![], vec![]),
        Expr::Enum(
            vec![Variant::new("a", vec![]), Variant::new("b", vec![name("u8"), name("bool")])],
            vec![],
        ),
        Expr::Enum(vec![Variant::new("a", vec![])], vec![Member::Item(Bind::new("n", int(1)).item())]),
        Expr::Enum(vec![], vec![Member::Item(Bind::new("n", int(1)).item())]),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn operators() {
    let borrowed = Borrow { qualifier: Some(BorrowWord::Temp), mode: Some(BorrowWord::Mut) };
    for expr in [
        Expr::member(name("a"), "b"),
        Expr::field_at(name("t"), 0),
        Expr::field_at(Expr::field_at(name("t"), 0), 1),
        Expr::member(int(1), "max"),
        Expr::method(int(-1), "abs", vec![]),
        Expr::method(Expr::float(1.5), "floor", vec![]),
        Expr::method(name("x"), "to", vec![name("u8")]),
        Expr::index(name("xs"), int(0)),
        Expr::call(Expr::index(name("fs"), int(0)), vec![int(1), int(2)]),
        Expr::prefix(TokenKind::Minus, name("x")),
        Expr::prefix(TokenKind::Minus, Expr::prefix(TokenKind::Minus, name("x"))),
        Expr::prefix(TokenKind::Bang, Expr::infix(TokenKind::And, name("a"), name("b"))),
        Expr::postfix(TokenKind::DotStar, name("p")),
        Expr::postfix(TokenKind::Question, Expr::call(name("f"), vec![])),
        Expr::postfix(TokenKind::DotStar, Expr::prefix(TokenKind::Minus, name("p"))),
        Expr::borrow(Borrow::SHARED, name("x")),
        Expr::borrow(Borrow::MUT, Expr::member(name("x"), "y")),
        Expr::borrow(borrowed, name("x")),
        Expr::Infix(TokenKind::DotDot, Box::new(int(1)), None),
        Expr::infix(TokenKind::DotDotEq, int(0), name("n")),
        Expr::assign(TokenKind::PlusEq, name("x"), int(1)),
        Expr::assign(TokenKind::Eq, Expr::postfix(TokenKind::DotStar, name("p")), int(1)),
        Expr::prefix(TokenKind::Unsafe, Expr::block(vec![], Some(int(1)))),
        Expr::prefix(TokenKind::Dollar, Expr::call(name("f"), vec![])),
        Expr::prefix(TokenKind::Dollar, Expr::when(name("c"), block(vec![], Some(int(1))), None)),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn control() {
    let two = block(vec![], Some(int(2)));
    let guarded =
        Arm { guard: Some(Expr::infix(TokenKind::Gt, name("x"), int(0))), ..arm(name("x"), int(1)) };
    for expr in [
        Expr::block(vec![], None),
        Expr::block(vec![Expr::bind(Bind::new("a", int(1)))], Some(name("a"))),
        Expr::when(name("c"), two.clone(), None),
        Expr::when(name("c"), two.clone(), Some(Expr::Block(two.clone()))),
        Expr::when(
            Expr::Test(Box::new(Expr::variant("some", vec![name("v")])), Box::new(name("o"))),
            two.clone(),
            Some(Expr::when(name("d"), two.clone(), Some(Expr::Block(two.clone())))),
        ),
        Expr::For(None, block(vec![Expr::Break(None, None)], None)),
        Expr::For(Some(Box::new(Expr::infix(TokenKind::Lt, name("i"), int(3)))), block(vec![], None)),
        Expr::For(
            Some(Box::new(Expr::In(
                Box::new(name("x")),
                Box::new(Expr::infix(TokenKind::DotDot, int(0), int(3))),
            ))),
            block(vec![Expr::Continue(None)], None),
        ),
        Expr::Labeled(
            "outer".to_owned(),
            Box::new(Expr::For(
                None,
                block(vec![Expr::Break(Some("outer".to_owned()), Some(Box::new(int(1))))], None),
            )),
        ),
        Expr::matching(name("x"), vec![arm(int(0), int(1)), guarded, arm(Expr::Hole, int(2))]),
        Expr::matching(name("x"), vec![]),
        Expr::Return(None),
        Expr::Return(Some(Box::new(int(1)))),
        Expr::Yield(Box::new(int(1))),
        Expr::Defer(Box::new(Expr::assign(TokenKind::Eq, name("x"), int(0)))),
        Expr::block(vec![Expr::Defer(Box::new(Expr::call(name("f"), vec![])))], None),
        Expr::List(vec![Expr::Staged(Box::new(Expr::For(
            Some(Box::new(Expr::In(Box::new(name("i")), Box::new(name("xs"))))),
            Block::of(name("i")),
        )))]),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn a_file_of_statements() {
    let body = block(
        vec![
            Expr::bind(Bind::mutable("n", name("u32"), int(0))),
            Expr::bind(Bind { mutable: true, pattern: name("buf"), ty: Some(name("T")), value: None }),
            Expr::bind(Bind::new(
                "x",
                Expr::when(name("c"), Block::of(int(1)), Some(Expr::Block(Block::of(int(2))))),
            )),
            Expr::when(name("c"), block(vec![], None), None),
            Expr::assign(TokenKind::PlusEq, name("n"), int(1)),
            Expr::call(name("f"), vec![Expr::postfix(TokenKind::DotStar, name("p"))]),
        ],
        Some(name("n")),
    );
    let file = File {
        doc: vec![],
        items: vec![
            Bind::new("main", Expr::Function(Function { params: vec![], result: None, body })).item(),
        ],
    };
    let kinds = read(&file.source());
    assert_eq!(kinds[0], NodeKind::File);
}
