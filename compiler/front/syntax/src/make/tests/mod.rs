mod forms;
mod items;
mod precedence;

use vun_diag::{DiagCtxt, DiagCx};
use vun_grammar::NodeKind;
use vun_origin::OriginTable;
use vun_span::SourceFiles;

use super::Expr;
use crate::cst::Cst;
use crate::tokens::Tokens;

/// The kinds of the nodes `text` is read as, in preorder, parentheses and tokens left out; the
/// text must parse without a report and its tree verify.
fn read(text: &str) -> Vec<NodeKind> {
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    assert_eq!(diag.error_count(), 0, "{text}");
    cst.verify(&tokens, text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    let mut kinds = Vec::new();
    let mut stack = vec![cst.root()];
    while let Some(n) = stack.pop() {
        kinds.push(cst.kind(n));
        stack.extend(cst.children(n).rev());
    }
    kinds.retain(|k| !matches!(k, NodeKind::Paren | NodeKind::Punct | NodeKind::Skipped));
    kinds
}

/// What `expr` prints as, read back as a file binding it: the printed text and its kinds below
/// the file and the binding.
fn round_trip(expr: &Expr) -> (String, Vec<NodeKind>) {
    // A form looser than a value, as an assignment is, is a statement of a block.
    let held =
        if expr.level() < Expr::value_level() { Expr::block(vec![expr.clone()], None) } else { expr.clone() };
    let text = format!("x := {};", held.source());
    let mut kinds = read(&text);
    assert_eq!(kinds.drain(..2).collect::<Vec<_>>(), [NodeKind::File, NodeKind::Binding], "{text}");
    assert_eq!(kinds.remove(0), NodeKind::Name, "{text}");
    if matches!(held, Expr::Block(_)) && !matches!(expr, Expr::Block(_)) {
        assert_eq!(kinds.remove(0), NodeKind::Block, "{text}");
    }
    (text, kinds)
}

/// The text of `expr` reads back as the kinds `expr` says it is made of.
#[track_caller]
fn assert_reads_back(expr: &Expr) {
    let (text, kinds) = round_trip(expr);
    assert_eq!(kinds, expr.kinds(), "{text}");
}
