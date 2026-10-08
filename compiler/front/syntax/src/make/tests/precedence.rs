//! Parentheses appear where the grammar's levels and head and statement rules ask, and nowhere else.

use vun_grammar::TokenKind;

use super::assert_reads_back;
use crate::make::{Block, Expr, Init};

fn name(n: &str) -> Expr {
    Expr::local(n)
}

fn op(token: TokenKind, a: Expr, b: Expr) -> Expr {
    Expr::infix(token, a, b)
}

#[test]
fn infix_operands_group_as_written() {
    let (a, b, c) = (name("a"), name("b"), name("c"));
    for expr in [
        op(TokenKind::Minus, a.clone(), op(TokenKind::Minus, b.clone(), c.clone())),
        op(TokenKind::Minus, op(TokenKind::Minus, a.clone(), b.clone()), c.clone()),
        op(TokenKind::Star, op(TokenKind::Plus, a.clone(), b.clone()), c.clone()),
        op(TokenKind::Plus, a.clone(), op(TokenKind::Star, b.clone(), c.clone())),
        op(TokenKind::Lt, op(TokenKind::Lt, a.clone(), b.clone()), c.clone()),
        op(TokenKind::EqEq, a.clone(), op(TokenKind::EqEq, b.clone(), c.clone())),
        op(TokenKind::Or, op(TokenKind::And, a.clone(), b.clone()), c.clone()),
        op(TokenKind::And, a.clone(), op(TokenKind::Or, b.clone(), c.clone())),
        op(TokenKind::DotDot, op(TokenKind::Plus, a.clone(), b.clone()), c.clone()),
        op(TokenKind::Shl, a.clone(), op(TokenKind::Pipe, b.clone(), c.clone())),
        op(TokenKind::Minus, a.clone(), Expr::prefix(TokenKind::Minus, b.clone())),
        op(TokenKind::Minus, a.clone(), Expr::int(-1)),
        op(
            TokenKind::Plus,
            Expr::when(a.clone(), Block::of(b.clone()), Some(Expr::Block(Block::of(c.clone())))),
            c.clone(),
        ),
        op(TokenKind::Plus, Expr::Return(Some(Box::new(a.clone()))), c.clone()),
        Expr::prefix(TokenKind::Minus, op(TokenKind::Plus, a.clone(), b.clone())),
        Expr::method(op(TokenKind::Plus, a.clone(), b.clone()), "abs", vec![]),
        Expr::method(Expr::prefix(TokenKind::Minus, a.clone()), "abs", vec![]),
        Expr::index(Expr::Block(Block::of(a.clone())), b.clone()),
        Expr::call(
            Expr::Function(crate::make::Function {
                params: vec![],
                result: None,
                body: Block::of(a.clone()),
            }),
            vec![],
        ),
        Expr::postfix(TokenKind::Question, Expr::postfix(TokenKind::DotStar, a)),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn a_brace_in_a_head_is_parenthesized() {
    let point = Expr::Typed(Box::new(name("Point")), vec![Init::Named("x".to_owned(), Expr::int(0))]);
    let condition = op(TokenKind::EqEq, name("p"), point.clone());
    let body = Block::of(Expr::int(1));
    for expr in [
        Expr::when(condition.clone(), body.clone(), None),
        Expr::For(Some(Box::new(condition.clone())), body.clone()),
        Expr::matching(point, vec![]),
        Expr::when(Expr::Block(Block::of(name("c"))), body.clone(), None),
        Expr::when(
            Expr::when(name("c"), Block::of(name("d")), Some(Expr::Block(Block::of(name("e"))))),
            body,
            None,
        ),
        Expr::Function(crate::make::Function {
            params: vec![],
            result: Some(Box::new(Expr::Typed(Box::new(name("T")), vec![]))),
            body: Block::default(),
        }),
    ] {
        assert_reads_back(&expr);
    }
}

#[test]
fn a_statement_starting_with_a_form_ending_in_a_brace_is_read_whole() {
    let block = Expr::Block(Block::of(Expr::int(1)));
    let stmts = vec![
        Expr::method(Expr::Block(Block::of(name("a"))), "f", vec![]),
        op(
            TokenKind::Plus,
            Expr::when(name("c"), Block::of(name("a")), Some(Expr::Block(Block::of(name("b"))))),
            Expr::int(1),
        ),
        block,
        Expr::int(2),
    ];
    assert_reads_back(&Expr::Block(Block::new(stmts, None)));
}
