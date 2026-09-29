//! A file's syntax tree as the compiler's parser builds it and as the tree-sitter grammar does,
//! compared node by node: each named node by its kind and the bytes it covers. Tree-sitter names
//! the tokens a branch reads by hand after their kind (`doc_comment`), where the compiler's tree
//! has no node, and cannot tell apart kinds with one production (`typed_binding` and `field`),
//! which are the same kind here.

use vun_diag::{DiagCtxt, DiagCx};
use vun_grammar::{NodeKind, TokenKind};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;
use vun_tree_sitter::{Node, Point};

/// The kind tree-sitter gives what it could not parse.
const ERROR: &str = "ERROR";

/// A node of either tree: its kind's name and the bytes it covers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Spanned {
    pub(crate) kind: &'static str,
    /// The bytes it covers: where they start and end.
    pub(crate) bytes: (u32, u32),
}

/// How the two trees of a file differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Difference {
    /// Tree-sitter reported an error or supposed a missing token at these bytes.
    Error((u32, u32)),
    /// A node of the compiler's tree tree-sitter has not, or has otherwise.
    Missing(Spanned),
    /// A node of tree-sitter's the compiler's tree has not.
    Extra(Spanned),
}

/// Where each line of `text` starts.
fn line_starts(text: &str) -> Vec<u32> {
    let starts = text.match_indices('\n').map(|(i, _)| u32::try_from(i + 1).unwrap_or(u32::MAX));
    std::iter::once(0).chain(starts).collect()
}

fn byte(starts: &[u32], p: Point) -> u32 {
    starts.get(p.row as usize).copied().unwrap_or_default() + p.column
}

/// The name of a kind as both trees know it: the first of the kinds sharing its production.
fn canonical(kind: NodeKind) -> &'static str {
    let same = NodeKind::ALL.iter().find(|k| k.production().is_some() && k.production() == kind.production());
    same.copied().unwrap_or(kind).name()
}

/// The compiler's nodes of `text`, but punctuation.
fn compiler(text: &str) -> Vec<Spanned> {
    let file = SourceFiles::new().add("compared.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    let spanned = cst.ids().filter(|&n| cst.kind(n) != NodeKind::Punct).map(|n| {
        let range = cst.span(n, &tokens).range;
        Spanned { kind: canonical(cst.kind(n)), bytes: (range.lo, range.hi) }
    });
    spanned.collect()
}

/// Tree-sitter's named nodes of `root`, but those naming a token, with the errors it reported.
fn grammar(root: &Node, starts: &[u32], nodes: &mut Vec<Spanned>, errors: &mut Vec<(u32, u32)>) {
    let bytes = (byte(starts, root.start), byte(starts, root.end));
    if root.missing || root.kind == ERROR {
        errors.push(bytes);
    }
    let token = TokenKind::ALL.iter().any(|k| k.name() == root.kind);
    if let Some(kind) = NodeKind::ALL.iter().find(|k| k.name() == root.kind) {
        nodes.push(Spanned { kind: canonical(*kind), bytes });
    } else if !token && !root.missing && root.kind != ERROR {
        // A named node of neither grammar: say so as a node the compiler's tree lacks.
        nodes.push(Spanned { kind: "unknown", bytes });
    }
    for child in &root.children {
        grammar(child, starts, nodes, errors);
    }
}

/// How the trees of `text` differ; none when they agree.
pub(crate) fn differences(text: &str, root: &Node) -> Vec<Difference> {
    let starts = line_starts(text);
    let (mut theirs, mut errors) = (Vec::new(), Vec::new());
    grammar(root, &starts, &mut theirs, &mut errors);
    let mut ours = compiler(text);
    ours.sort();
    theirs.sort();
    let mut found: Vec<Difference> = errors.into_iter().map(Difference::Error).collect();
    found.extend(ours.iter().filter(|n| theirs.binary_search(n).is_err()).cloned().map(Difference::Missing));
    // A token a branch reads by hand is a node of tree-sitter's, named after its kind, alone.
    let token = |n: &Spanned| TokenKind::ALL.iter().any(|k| k.name() == n.kind);
    let extra = theirs.iter().filter(|n| ours.binary_search(n).is_err() && !token(n));
    found.extend(extra.cloned().map(Difference::Extra));
    found
}
