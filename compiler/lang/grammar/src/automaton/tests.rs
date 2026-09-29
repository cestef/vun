use crate::{NodeField, NodeKind, PARTS, TokenKind};

#[test]
fn forms_beginning_with_an_operand_continue_one() {
    assert!(NodeKind::Call.continues() && NodeKind::Infix.continues() && NodeKind::Binding.continues());
    assert!(!NodeKind::Paren.continues() && !NodeKind::Prefix.continues());
    // `mut x := 1` starts alone with the `mut` its first child reads.
    assert!(NodeKind::Binding.starts_alone().contains(TokenKind::Mut));
    assert!(!NodeKind::Binding.starts_alone().contains(TokenKind::Ident));
    assert!(NodeKind::Paren.starts_alone().contains(TokenKind::LParen));
}

#[test]
fn operands_are_neither_parts_nor_roots() {
    let operands = NodeKind::operands();
    assert!(PARTS.iter().all(|&k| !operands.contains(k)));
    assert!(NodeKind::File.is_root() && !operands.contains(NodeKind::File));
    assert!(!operands.contains(NodeKind::Error) && operands.contains(NodeKind::Call));
    assert!(NodeKind::Call.first().contains(TokenKind::Ident));
}

#[test]
fn separated_terms_may_end_after_a_separator() {
    // `.tuple("(", .separated(.items, ","), ")")`: `(`, items, `,`, `)`.
    let tuple = NodeKind::Tuple.automaton().unwrap();
    let open = tuple.next(None);
    let item = tuple.next(Some(open)).filter(|p| p == 1);
    let comma = tuple.next(Some(item)).filter(|p| p == 2);
    assert!(tuple.next(Some(comma)).contains(3) && tuple.next(Some(comma)).contains(1));
    assert!(tuple.accepts(Some(tuple.next(Some(open)).filter(|p| p == 3))));
    assert!(tuple.anchors.contains(0) && !tuple.anchors.contains(2));
}

#[test]
fn fields_lead_to_what_they_admit() {
    assert!(NodeField::Params.leading().contains(NodeKind::Param));
    assert!(!NodeField::Items.leading().contains(NodeKind::Param));
}

#[test]
fn kinds_index_their_automata() {
    for (i, &kind) in NodeKind::ALL.iter().enumerate() {
        assert_eq!(kind.index(), i);
    }
    assert!(!NodeKind::Name.continues());
}
