//! Error-recovering recursive descent producing the CST.
//!
//! Patterns are parsed as expressions: they mirror constructors, so `(a, b)`, `.some(x)` and
//! `.{ .x = p }` have one shape in both roles, and later stages decide which role a node plays.
//! Postorder lets the parser choose a node's kind after its children: a parenthesized group becomes
//! a function once `->` or a body follows it, a member becomes a field once `,` or `}` follows it.
//!
//! Every error is reported once, where it is found, and the tree keeps its shape: absent tokens and
//! operands become nodes anchored where they were expected, and skipped tokens become `Error`
//! leaves.

use vun_diag::DiagCx;
use vun_trace::tracing;

use crate::cst::{Builder, Cst};
use crate::tokens::Tokens;

mod asm;
mod control;
mod cursor;
mod entries;
mod expr;
mod lists;
mod nesting;
mod recovery;
mod report;
#[cfg(test)]
mod tests;

/// How deeply expressions and blocks may nest before the parser gives up on a subtree.
const MAX_NESTING: u32 = 256;

impl Cst {
    /// Parses one file. Diagnostics go to `cx`; the result is always a complete tree.
    pub fn parse(tokens: &Tokens, source: &str, cx: &mut DiagCx<'_>) -> Self {
        let _span = tracing::debug_span!("parse", file = ?tokens.file(), tokens = tokens.len()).entered();
        let mut parser =
            Parser { tokens, source, pos: 0, depth: 0, reported: None, out: Builder::new(tokens.len()), cx };
        parser.file();
        let cst = parser.out.finish();
        tracing::debug!(nodes = cst.len(), "parsed");
        cst
    }
}

/// Whether a `{` after an operand continues the expression. In the head of `if`, `for` and
/// `match`, and in a result type, it opens the body instead. Among an instruction's operands, it
/// opens a Vun value bound to one (`{in e}`), and `[` a memory operand.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Brace {
    Continues,
    Ends,
    Hole,
}

/// What a list holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    /// Declarations and fields: files and `struct` bodies.
    Members,
    /// Statements and a final value: blocks and quotes.
    Statements,
    /// Comma-separated items.
    Items(Item),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    /// An expression, or `name = value` in literals and arguments.
    Value,
    /// A value; the first may be followed by `; count` to repeat it.
    Element,
    /// A value, or `pattern: type` in a parameter list.
    Param,
    /// `pattern [if guard] => value`.
    Arm,
    /// A plain name, as selected by a spread: `..ns.{ a, b }`.
    Name,
}

/// How an entry of a list ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    /// A binding or a statement: `;`.
    Semi,
    /// A field: `,`, or nothing before the closing token.
    Comma,
    /// An expression: `;`, or nothing when it is the value of its block.
    Value,
    /// A block-like expression: an optional `;`.
    Block,
    /// Nothing; the entry was an error and recovery already ran.
    Recovered,
}

/// Whether an item list repeated its only element: `[v; n]`.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Form {
    #[default]
    Listed,
    Repeated,
}

/// What an item list contained, for choosing the kind of the node that owns it.
#[derive(Clone, Copy, Default)]
struct Shape {
    items: usize,
    commas: usize,
    typed: usize,
    form: Form,
}

impl Shape {
    fn single(self) -> bool {
        self.items == 1 && self.commas == 0 && self.form == Form::Listed
    }
}

/// Whether a jump takes an operand.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operand {
    Never,
    Optional,
    Required,
}

struct Parser<'t, 'c, 'd> {
    tokens: &'t Tokens,
    source: &'t str,
    pos: usize,
    depth: u32,
    /// The position of the last error, so that one position reports once.
    reported: Option<usize>,
    out: Builder,
    cx: &'c mut DiagCx<'d>,
}
