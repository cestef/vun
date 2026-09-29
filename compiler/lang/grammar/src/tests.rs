use super::*;
use std::collections::HashSet;

#[test]
fn spellings_are_unique() {
    let fixed = TokenKind::KEYWORDS.iter().chain(TokenKind::PUNCTUATION);
    let texts: Vec<_> = fixed.map(|k| k.text().unwrap()).collect();
    assert_eq!(texts.iter().collect::<HashSet<_>>().len(), texts.len());
}

#[test]
fn keyword_lookup_round_trips() {
    for &kw in TokenKind::KEYWORDS {
        assert_eq!(TokenKind::keyword(kw.text().unwrap()), Some(kw));
    }
    assert_eq!(TokenKind::keyword("fn"), None);
}

#[test]
fn punctuation_takes_the_longest_match() {
    assert_eq!(TokenKind::punctuation("+%= 1"), Some(TokenKind::PlusPercentEq));
    assert_eq!(TokenKind::punctuation("..= b"), Some(TokenKind::DotDotEq));
    assert_eq!(TokenKind::punctuation(".. b"), Some(TokenKind::DotDot));
    assert_eq!(TokenKind::punctuation("@"), None);
}

#[test]
fn operator_tables_are_disjoint_where_they_must_be() {
    for row in INFIX {
        for k in row.tokens {
            assert!(!ASSIGN.contains(k), "{k:?}");
        }
    }
    let mul = TokenKind::Star.infix().unwrap();
    let add = TokenKind::Plus.infix().unwrap();
    assert!(mul.level > add.level);
    assert_eq!(TokenKind::Lt.infix().unwrap().assoc, Assoc::None);
    assert_eq!(TokenKind::Dot.infix(), None);
}

#[test]
fn leaves_spell_one_token_and_hold_nothing() {
    for &k in NodeKind::LEAVES {
        assert!(k.is_leaf() && k.slots().is_empty(), "{k:?}");
        assert!(
            k.production().is_none_or(|p| matches!(p, [NodeTerm::Token(_) | NodeTerm::Word(_)])),
            "{k:?}"
        );
    }
}

fn token(kind: TokenKind) -> Element<'static> {
    Element::Token { kind, text: kind.text().unwrap_or_default() }
}

#[test]
fn productions_match_as_regular_expressions() {
    use Element::Child;
    let (l, r, comma) = (token(TokenKind::LParen), token(TokenKind::RParen), token(TokenKind::Comma));
    let (callee, arg) = (
        Child {
            field: NodeField::Callee,
            kind: NodeKind::Name,
            ends: NodeKind::Name,
            first: TokenKind::Ident,
        },
        Child { field: NodeField::Args, kind: NodeKind::Name, ends: NodeKind::Name, first: TokenKind::Ident },
    );
    // `.call(.callee, "(", .separated(.args, ","), ")")`
    assert!(NodeKind::Call.spelled_by(&[callee, l, r]));
    assert!(NodeKind::Call.spelled_by(&[callee, l, arg, comma, arg, r]));
    assert!(NodeKind::Call.spelled_by(&[callee, l, arg, comma, r]));
    assert!(!NodeKind::Call.spelled_by(&[callee, l, arg, arg, r]));
    assert!(!NodeKind::Call.spelled_by(&[callee, l, comma, r]));
    assert!(!NodeKind::Call.spelled_by(&[l, arg, r]));
    // `.in(.pattern, "in", .value)`: a word is a name spelled so.
    let word = |text| Element::Token { kind: TokenKind::Ident, text };
    let (pattern, value) = (
        Child {
            field: NodeField::Pattern,
            kind: NodeKind::Name,
            ends: NodeKind::Name,
            first: TokenKind::Ident,
        },
        Child {
            field: NodeField::Value,
            kind: NodeKind::Name,
            ends: NodeKind::Name,
            first: TokenKind::Ident,
        },
    );
    assert!(NodeKind::In.spelled_by(&[pattern, word("in"), value]));
    assert!(!NodeKind::In.spelled_by(&[pattern, word("on"), value]));
}

#[test]
fn long_repetitions_match_without_recursing() {
    let (statement, semi) = (
        Element::Child {
            field: NodeField::Statements,
            kind: NodeKind::Name,
            ends: NodeKind::Name,
            first: TokenKind::Ident,
        },
        token(TokenKind::Semi),
    );
    let mut elements = vec![token(TokenKind::LBrace)];
    elements.extend([statement, semi].repeat(100_000));
    elements.push(token(TokenKind::RBrace));
    assert!(NodeKind::Block.spelled_by(&elements));
}

#[test]
fn entries_end_with_a_semicolon_unless_block_like() {
    let (open, close, semi) = (token(TokenKind::LBrace), token(TokenKind::RBrace), token(TokenKind::Semi));
    let value = Element::Child {
        field: NodeField::Statements,
        kind: NodeKind::Name,
        ends: NodeKind::Name,
        first: TokenKind::Ident,
    };
    let branch = Element::Child {
        field: NodeField::Statements,
        kind: NodeKind::If,
        ends: NodeKind::If,
        first: TokenKind::Ident,
    };
    assert!(NodeKind::Block.spelled_by(&[open, value, semi, branch, branch, value, close]));
    assert!(NodeKind::Block.spelled_by(&[open, close]));
    assert!(!NodeKind::Block.spelled_by(&[open, value, value, close]));
}

#[test]
fn recovery_shapes_fit_as_left() {
    let (l, r) = (token(TokenKind::LParen), token(TokenKind::RParen));
    let child = |kind| Element::Child { field: NodeField::Inner, kind, ends: kind, first: TokenKind::Ident };
    // `(a`: the absent `)` stands for its term; `(a b)`: the skipped `b` for nothing.
    assert!(NodeKind::Paren.spelled_by(&[l, child(NodeKind::Name), Element::Absent]));
    assert!(NodeKind::Paren.spelled_by(&[l, child(NodeKind::Name), child(NodeKind::Error), r]));
    assert!(!NodeKind::Paren.spelled_by(&[l, child(NodeKind::Name), child(NodeKind::Name), r]));
}

#[test]
fn fields_admit_parts_where_tables_say() {
    assert!(NodeKind::FnType.admits(NodeField::Params, NodeKind::Param, &[]));
    assert!(!NodeKind::Tuple.admits(NodeField::Items, NodeKind::Param, &[]));
    assert!(NodeKind::Tuple.admits(NodeField::Items, NodeKind::Error, &[]));
    assert!(NodeKind::Fn.admits(NodeField::Returns, NodeKind::Returns, &[]));
    assert!(!NodeKind::If.admits(NodeField::Otherwise, NodeKind::Name, &[]));
    assert!(!NodeKind::Paren.admits(NodeField::Inner, NodeKind::Bound, &[]));
    assert!(NodeKind::Paren.admits(NodeField::Inner, NodeKind::Bound, &[NodeKind::Bound]));
}

#[test]
fn groups_try_the_preferred_kinds_first() {
    let kinds = NodeKind::starting_with(TokenKind::LParen);
    let at = |kind| kinds.iter().position(|&k| k == kind);
    assert!(at(NodeKind::Paren) < at(NodeKind::Tuple) && at(NodeKind::Tuple) < at(NodeKind::FnType));
    assert!(at(NodeKind::Fn) < at(NodeKind::Tuple));
}

#[test]
fn continuations_are_told_apart_by_their_leading_tokens() {
    for c in Continuation::all() {
        assert!(c.leads.iter().all(|run| !run.is_empty()), "{:?}", c.kind);
    }
    let longest = |kind| {
        Continuation::all()
            .iter()
            .find(|c| c.kind == kind)
            .and_then(|c| c.leads.iter().map(|r| r.len()).max())
    };
    // `.asm {` spells `.` and more: where both are written, the block wins.
    assert!(longest(NodeKind::Asm) > longest(NodeKind::Member));
    assert!(Continuation::all().iter().all(|c| !PARTS.contains(&c.kind)));
}
