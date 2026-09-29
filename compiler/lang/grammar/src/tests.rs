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
    assert!(mul.prec > add.prec);
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
    let (callee, arg) = (Child(NodeField::Callee), Child(NodeField::Args));
    // `.call(.callee, "(", .separated(.args, ","), ")")`
    assert!(NodeKind::Call.spelled_by(&[callee, l, r]));
    assert!(NodeKind::Call.spelled_by(&[callee, l, arg, comma, arg, r]));
    assert!(NodeKind::Call.spelled_by(&[callee, l, arg, comma, r]));
    assert!(!NodeKind::Call.spelled_by(&[callee, l, arg, arg, r]));
    assert!(!NodeKind::Call.spelled_by(&[callee, l, comma, r]));
    assert!(!NodeKind::Call.spelled_by(&[l, arg, r]));
    // `.in(.pattern, "in", .value)`: a word is a name spelled so.
    let word = |text| Element::Token { kind: TokenKind::Ident, text };
    let (pattern, value) = (Child(NodeField::Pattern), Child(NodeField::Value));
    assert!(NodeKind::In.spelled_by(&[pattern, word("in"), value]));
    assert!(!NodeKind::In.spelled_by(&[pattern, word("on"), value]));
}

#[test]
fn long_repetitions_match_without_recursing() {
    let (statement, semi) = (Element::Child(NodeField::Statements), token(TokenKind::Semi));
    let mut elements = vec![token(TokenKind::LBrace)];
    elements.extend([statement, semi].repeat(100_000));
    elements.push(token(TokenKind::RBrace));
    assert!(NodeKind::Block.spelled_by(&elements));
}
