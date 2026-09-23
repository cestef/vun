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
    for &(_, kinds) in INFIX {
        for k in kinds {
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
fn node_kinds_are_leaves_or_branches() {
    assert!(NodeKind::LEAVES.iter().all(|k| k.is_leaf()));
    assert!(NodeKind::BRANCHES.iter().all(|k| !k.is_leaf()));
}
