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

use overt_diag::{ChildKind, DiagCx, DiagId, Diagnostic, Msg, errors};
use overt_grammar::{
    ASSIGN, Assoc, Context, ELEMENT_TYPES, LITERAL_TYPES, NodeKind, OPERANDS, POSTFIX, PREFIX, Prec,
    TokenKind, contextual,
};
use overt_index::Idx;
use overt_span::{ByteRange, Span};
use overt_trace::tracing;

use crate::cst::{Anchor, Builder, Cst};
use crate::tokens::{TokenId, Tokens};

/// How deeply expressions and blocks may nest before the parser gives up on a subtree.
const MAX_NESTING: u32 = 256;

/// Parses one file. Diagnostics go to `cx`; the result is always a complete tree.
pub fn parse(tokens: &Tokens, source: &str, cx: &mut DiagCx<'_>) -> Cst {
    let _span = tracing::debug_span!("parse", file = ?tokens.file(), tokens = tokens.len()).entered();
    let mut parser =
        Parser { tokens, source, pos: 0, depth: 0, reported: None, out: Builder::new(tokens.len()), cx };
    parser.file();
    let cst = parser.out.finish();
    tracing::debug!(nodes = cst.len(), "parsed");
    cst
}

/// Whether a `{` after an operand continues the expression. In the head of `if`, `for` and
/// `match`, and in a result type, it opens the body instead.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Brace {
    Continues,
    Ends,
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

impl Parser<'_, '_, '_> {
    fn file(&mut self) {
        let mark = self.out.mark();
        self.list(List::Members, TokenKind::Eof);
        self.out.node(NodeKind::File, Anchor::Token(self.current()), mark);
    }

    // Lists.

    /// Entries up to `close`, which is left for the caller.
    fn list(&mut self, list: List, close: TokenKind) -> Shape {
        let mut shape = Shape::default();
        while !self.at(close) && !self.at(TokenKind::Eof) {
            let before = self.pos;
            let proceed = if let List::Items(item) = list {
                self.item(item, close, &mut shape)
            } else {
                self.entry(list, close);
                true
            };
            if self.pos == before && !self.at(close) && !self.at(TokenKind::Eof) {
                self.report_expected_entry(list);
                self.leaf(NodeKind::Error);
            }
            if !proceed {
                break;
            }
        }
        shape
    }

    /// A braced list, the `{` being a punctuation leaf of the enclosing node.
    fn braced(&mut self, list: List) {
        let open = self.current();
        if self.eat(TokenKind::LBrace) {
            self.list(list, TokenKind::RBrace);
            self.close(TokenKind::RBrace, open);
        } else {
            self.report_expected(&[TokenKind::LBrace]);
            self.missing(NodeKind::Punct);
        }
    }

    /// `{ list }` as a `Block` node.
    fn block(&mut self, list: List) {
        if !self.at(TokenKind::LBrace) {
            self.report_expected(&[TokenKind::LBrace]);
            self.missing(NodeKind::Error);
            return;
        }
        self.nested(|p| {
            let mark = p.out.mark();
            let open = p.bump();
            p.list(list, TokenKind::RBrace);
            p.close(TokenKind::RBrace, open);
            p.out.node(NodeKind::Block, Anchor::Token(open), mark);
        });
    }

    /// The closing token of a group opened by `open`.
    fn close(&mut self, close: TokenKind, open: TokenId) {
        if self.eat(close) {
            return;
        }
        let opener = self.text(open).to_owned();
        let msg = Msg::new().text("unclosed ").code(opener);
        let help = Msg::new().text("expected ").code(close.text().unwrap_or_default()).text(", found ");
        let d = Diagnostic::error(errors::SYNTAX_UNCLOSED_DELIMITER, msg)
            .child(ChildKind::Help, self.found(help));
        self.report(self.tokens.span(open), d);
        self.missing(NodeKind::Punct);
    }

    /// One entry of a member or statement list, with its terminator.
    fn entry(&mut self, list: List, close: TokenKind) {
        if self.at(TokenKind::ModuleDocComment) && list == List::Members {
            self.leaf(NodeKind::ModuleDoc);
            return;
        }
        if self.eat(TokenKind::Semi) {
            return;
        }
        match self.annotated(|p| p.entry_body(list, close)) {
            End::Semi => self.terminator(),
            End::Comma => {
                if !self.eat(TokenKind::Comma) && !self.at(close) {
                    self.report_expected(&[TokenKind::Comma, close]);
                }
            }
            End::Value => {
                if !self.at(close) {
                    self.terminator();
                }
            }
            End::Block => {
                self.eat(TokenKind::Semi);
            }
            End::Recovered => {}
        }
    }

    fn entry_body(&mut self, list: List, close: TokenKind) -> End {
        let statements = list == List::Statements;
        if self.block_like_at() && (statements || self.at(TokenKind::Dollar)) {
            self.block_like(list);
            return End::Block;
        }
        let mark = self.out.mark();
        let mutable = self.at(TokenKind::Mut);
        if mutable {
            self.leaf(NodeKind::Mut);
        } else if !self.starts_expr() {
            self.report_expected_entry(list);
            return self.recover_entry();
        }
        self.expr(Brace::Continues);
        match self.peek() {
            TokenKind::ColonEq => {
                let op = self.bump();
                self.expr(Brace::Continues);
                self.out.node(NodeKind::Binding, Anchor::Token(op), mark);
                End::Semi
            }
            TokenKind::Colon => {
                let op = self.bump();
                self.expr(Brace::Continues);
                if self.eat(TokenKind::Eq) {
                    self.expr(Brace::Continues);
                }
                let field = !statements && (self.at(TokenKind::Comma) || self.at(close));
                let kind = if field { NodeKind::Field } else { NodeKind::Binding };
                self.out.node(kind, Anchor::Token(op), mark);
                if field { End::Comma } else { End::Semi }
            }
            k if statements && !mutable && ASSIGN.contains(&k) => {
                let op = self.bump();
                self.expr(Brace::Continues);
                self.out.node(NodeKind::Assign, Anchor::Token(op), mark);
                End::Semi
            }
            _ if statements && !mutable => End::Value,
            _ => {
                self.report_expected(&[TokenKind::ColonEq, TokenKind::Colon]);
                self.recover_entry()
            }
        }
    }

    /// The `;` ending an entry. Tokens left on the same line are junk and skipped up to it; on a
    /// new line the `;` is missing and the next entry starts.
    fn terminator(&mut self) {
        if self.eat(TokenKind::Semi) {
            return;
        }
        self.report_expected(&[TokenKind::Semi]);
        if self.on_new_line() || self.at(TokenKind::Eof) {
            self.missing(NodeKind::Punct);
        } else {
            self.recover(&[TokenKind::Semi]);
            if !self.eat(TokenKind::Semi) {
                self.missing(NodeKind::Punct);
            }
        }
    }

    /// Whether a line break separates the current token from the previous one.
    fn on_new_line(&self) -> bool {
        let Some(previous) = self.pos.checked_sub(1) else { return true };
        let between = self.tokens.range(TokenId::new(previous)).hi..self.tokens.range(self.current()).lo;
        self.source[between.start as usize..between.end as usize].contains('\n')
    }

    fn recover_entry(&mut self) -> End {
        self.recover(&[TokenKind::Semi, TokenKind::Comma]);
        if !self.eat(TokenKind::Semi) {
            self.eat(TokenKind::Comma);
        }
        End::Recovered
    }

    /// One item and its separator; `false` ends the list.
    fn item(&mut self, item: Item, close: TokenKind, shape: &mut Shape) -> bool {
        if self.at(TokenKind::Dollar) && self.block_like_at() {
            self.block_like(List::Items(item));
        } else {
            self.annotated(|p| p.item_body(item, shape));
        }
        shape.items += 1;
        if self.eat(TokenKind::Comma) {
            shape.commas += 1;
            return true;
        }
        if item == Item::Element && shape.single() && self.eat(TokenKind::Semi) {
            self.expr(Brace::Continues);
            shape.form = Form::Repeated;
            return false;
        }
        let k = self.peek();
        if k == close || k == TokenKind::Semi || k.closes() || k == TokenKind::Eof {
            return false;
        }
        self.report_expected(&[TokenKind::Comma, close]);
        self.recover(&[TokenKind::Comma, TokenKind::Semi]);
        if self.eat(TokenKind::Comma) {
            shape.commas += 1;
            return true;
        }
        false
    }

    fn item_body(&mut self, item: Item, shape: &mut Shape) {
        let mark = self.out.mark();
        self.expr(Brace::Continues);
        match item {
            Item::Value | Item::Element => {
                if self.at(TokenKind::Eq) {
                    let op = self.bump();
                    self.expr(Brace::Continues);
                    self.out.node(NodeKind::FieldInit, Anchor::Token(op), mark);
                }
            }
            Item::Param => {
                if self.at(TokenKind::Colon) {
                    let op = self.bump();
                    self.expr(Brace::Continues);
                    self.out.node(NodeKind::Param, Anchor::Token(op), mark);
                    shape.typed += 1;
                }
            }
            Item::Arm => {
                if self.at(TokenKind::If) {
                    let guard = self.out.mark();
                    let token = self.bump();
                    self.expr(Brace::Continues);
                    self.out.node(NodeKind::Guard, Anchor::Token(token), guard);
                }
                let anchor = if self.at(TokenKind::FatArrow) {
                    Anchor::Token(self.bump())
                } else {
                    self.report_expected(&[TokenKind::FatArrow]);
                    Anchor::Missing(self.current())
                };
                if matches!(anchor, Anchor::Token(_)) || self.starts_expr() {
                    self.expr(Brace::Continues);
                }
                self.out.node(NodeKind::Arm, anchor, mark);
            }
        }
    }

    /// Doc comments and attributes, each wrapping what `body` parses after them.
    fn annotated<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        let mut open = Vec::new();
        loop {
            let mark = self.out.mark();
            match self.peek() {
                TokenKind::DocComment => open.push((NodeKind::Doc, self.bump(), mark)),
                TokenKind::HashLBracket => {
                    let token = self.bump();
                    self.list(List::Items(Item::Value), TokenKind::RBracket);
                    self.close(TokenKind::RBracket, token);
                    open.push((NodeKind::Attr, token, mark));
                }
                _ => break,
            }
        }
        if let Some(&(kind, token, _)) = open.last()
            && (self.peek().closes() || self.at(TokenKind::Eof))
        {
            let what = if kind == NodeKind::Doc { "doc comment" } else { "attribute" };
            let msg =
                self.found(Msg::new().text("expected a declaration after this ").text(what).text(", found "));
            let span = self.tokens.span(token);
            self.report(span, Diagnostic::error(errors::SYNTAX_EXPECTED_DECLARATION, msg));
        }
        let result = body(self);
        for (kind, token, mark) in open.into_iter().rev() {
            self.out.node(kind, Anchor::Token(token), mark);
        }
        result
    }

    // Expressions.

    fn expr(&mut self, brace: Brace) {
        self.binary(Prec::LOWEST, brace);
    }

    fn binary(&mut self, min: Prec, brace: Brace) {
        let mark = self.out.mark();
        self.prefix(brace);
        let mut chained = None;
        while let Some(op) = self.peek().infix().filter(|op| op.prec >= min) {
            if op.assoc == Assoc::None && chained == Some(op.prec) {
                self.report_chained();
            }
            let token = self.bump();
            // `a..` has no upper bound.
            if self.kind(token) != TokenKind::DotDot || self.starts_expr() {
                self.binary(op.prec.tighter(), brace);
            }
            self.out.node(NodeKind::Infix, Anchor::Token(token), mark);
            chained = (op.assoc == Assoc::None).then_some(op.prec);
        }
    }

    fn prefix(&mut self, brace: Brace) {
        self.nested(|p| {
            let k = p.peek();
            if k == TokenKind::HashLBracket {
                p.annotated(|p| p.prefix(brace));
            } else if PREFIX.contains(&k) {
                let mark = p.out.mark();
                let token = p.bump();
                if k == TokenKind::Amp {
                    if p.at(TokenKind::Mut) {
                        p.leaf(NodeKind::Mut);
                    } else if p.at_word(Context::AfterAmp) {
                        p.leaf(NodeKind::Mode);
                    }
                }
                p.prefix(brace);
                p.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
            } else {
                p.postfix(brace);
            }
        });
    }

    fn postfix(&mut self, brace: Brace) {
        let mark = self.out.mark();
        self.primary(brace);
        loop {
            let (kind, token) = match self.peek() {
                TokenKind::Dot => {
                    let token = self.bump();
                    self.member_name();
                    (NodeKind::Member, token)
                }
                k if POSTFIX.contains(&k) => (NodeKind::Postfix, self.bump()),
                TokenKind::LParen => (NodeKind::Call, self.group_of(List::Items(Item::Value)).0),
                TokenKind::LBracket => {
                    let token = self.bump();
                    self.expr(Brace::Continues);
                    self.close(TokenKind::RBracket, token);
                    (NodeKind::Index, token)
                }
                TokenKind::LBrace
                    if brace == Brace::Continues
                        && self.out.last().is_some_and(|k| LITERAL_TYPES.contains(&k)) =>
                {
                    (NodeKind::StructLit, self.group_of(List::Items(Item::Value)).0)
                }
                _ => break,
            };
            self.out.node(kind, Anchor::Token(token), mark);
        }
    }

    /// A delimited list whose opening token anchors the node the caller pushes.
    fn group_of(&mut self, list: List) -> (TokenId, Shape) {
        let open = self.bump();
        let close = self.kind(open).closer().expect("group_of starts at an opening delimiter");
        let shape = self.list(list, close);
        self.close(close, open);
        (open, shape)
    }

    fn member_name(&mut self) {
        match self.peek() {
            TokenKind::Ident | TokenKind::RawIdent => self.leaf(NodeKind::Name),
            TokenKind::Int => self.leaf(NodeKind::Int),
            TokenKind::Dollar => {
                let mark = self.out.mark();
                let token = self.bump();
                self.nested(|p| p.primary(Brace::Continues));
                self.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
            }
            _ => {
                let msg = self.found(Msg::new().text("expected a member name, found "));
                self.report_missing(errors::SYNTAX_EXPECTED_NAME, msg);
                self.missing(NodeKind::Error);
            }
        }
    }

    fn primary(&mut self, brace: Brace) {
        let leaf = match self.peek() {
            TokenKind::Ident | TokenKind::RawIdent => Some(NodeKind::Name),
            TokenKind::Underscore => Some(NodeKind::Hole),
            TokenKind::Int => Some(NodeKind::Int),
            TokenKind::Float => Some(NodeKind::Float),
            TokenKind::Char => Some(NodeKind::Char),
            TokenKind::String | TokenKind::RawString => Some(NodeKind::String),
            TokenKind::Error => Some(NodeKind::Error),
            _ => None,
        };
        if let Some(kind) = leaf {
            self.leaf(kind);
            return;
        }
        let mark = self.out.mark();
        let (kind, token) = match self.peek() {
            TokenKind::RawStringLine => {
                let first = self.bump();
                while self.at(TokenKind::RawStringLine) {
                    self.leaf(NodeKind::RawStringLine);
                }
                (NodeKind::MultilineString, first)
            }
            TokenKind::Dot => {
                let token = self.bump();
                self.member_name();
                (NodeKind::DotName, token)
            }
            TokenKind::DotLBrace => (NodeKind::StructLit, self.group_of(List::Items(Item::Value)).0),
            TokenKind::HashLBrace => (NodeKind::Quote, self.group_of(List::Statements).0),
            TokenKind::LParen => return self.function_or_tuple(brace),
            TokenKind::LBracket => return self.bracket(brace),
            TokenKind::LBrace | TokenKind::If | TokenKind::For | TokenKind::Match | TokenKind::Label => {
                return self.control(List::Statements);
            }
            TokenKind::Struct => {
                let token = self.bump();
                self.braced(List::Members);
                (NodeKind::Struct, token)
            }
            TokenKind::Enum => {
                let token = self.bump();
                self.braced(List::Items(Item::Value));
                (NodeKind::Enum, token)
            }
            TokenKind::Return
            | TokenKind::Break
            | TokenKind::Continue
            | TokenKind::Yield
            | TokenKind::Defer => return self.jump(brace),
            _ => {
                let msg = self.found(Msg::new().text("expected an expression, found "));
                let mut d = Diagnostic::error(errors::SYNTAX_EXPECTED_EXPRESSION, msg);
                if let Some(keyword) = self.nearby_keyword() {
                    let escaped = format!("\\{}", self.text(keyword));
                    let help = Msg::new().text("to use a keyword as a name, escape it: ").code(escaped);
                    d = d.child(ChildKind::Help, help);
                }
                self.report(self.gap(), d);
                self.missing(NodeKind::Error);
                return;
            }
        };
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// `(...)`: a tuple or a parenthesized expression; a function type when a parameter is typed or
    /// `->` follows; a function literal when a body follows either.
    fn function_or_tuple(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let (open, shape) = self.group_of(List::Items(Item::Param));
        let arrow = self.at(TokenKind::Arrow);
        if arrow {
            let returns = self.out.mark();
            let token = self.bump();
            self.expr(Brace::Ends);
            self.out.node(NodeKind::Returns, Anchor::Token(token), returns);
        }
        let body = brace == Brace::Continues
            && self.at(TokenKind::LBrace)
            && (arrow || shape.items == 0 || shape.typed > 0);
        if body {
            self.block(List::Statements);
        }
        let kind = if body {
            NodeKind::Fn
        } else if arrow || shape.typed > 0 {
            NodeKind::FnType
        } else if shape.single() {
            NodeKind::Paren
        } else {
            NodeKind::Tuple
        };
        self.out.node(kind, Anchor::Token(open), mark);
    }

    /// `[...]`: an array, a repeat, an array type `[N]T`, or the captures of a closure.
    fn bracket(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let (open, shape) = self.group_of(List::Items(Item::Element));
        let kind = if shape.form == Form::Repeated {
            NodeKind::ArrayRepeat
        } else if self.at(TokenKind::LParen) {
            let callee = self.out.mark();
            self.function_or_tuple(brace);
            let closure = self.out.single_since(callee) && self.out.last() == Some(NodeKind::Fn);
            if closure { NodeKind::Closure } else { NodeKind::ArrayType }
        } else if shape.single() && ELEMENT_TYPES.contains(&self.peek()) {
            self.prefix(brace);
            NodeKind::ArrayType
        } else {
            NodeKind::Array
        };
        self.out.node(kind, Anchor::Token(open), mark);
    }

    /// Block-like expressions: blocks, `if`, `for`, `match` and labeled loops, with bodies of `list`.
    fn control(&mut self, list: List) {
        let mark = self.out.mark();
        let (kind, token) = match self.peek() {
            TokenKind::LBrace => return self.block(list),
            TokenKind::If => {
                let token = self.bump();
                self.head(TokenKind::Eq);
                self.block(list);
                if self.eat(TokenKind::Else) {
                    if self.at(TokenKind::If) { self.control(list) } else { self.block(list) }
                }
                (NodeKind::If, token)
            }
            TokenKind::For => {
                let token = self.bump();
                if !self.at(TokenKind::LBrace) {
                    self.head(TokenKind::For);
                }
                self.block(list);
                (NodeKind::For, token)
            }
            TokenKind::Match => {
                let token = self.bump();
                self.expr(Brace::Ends);
                self.braced(List::Items(Item::Arm));
                (NodeKind::Match, token)
            }
            TokenKind::Label => {
                let token = self.bump();
                self.expect(TokenKind::Colon);
                if matches!(self.peek(), TokenKind::For | TokenKind::LBrace) {
                    self.control(list);
                } else {
                    self.report_expected(&[TokenKind::For, TokenKind::LBrace]);
                    self.missing(NodeKind::Error);
                }
                (NodeKind::Labeled, token)
            }
            _ => unreachable!("control called at a block-like token"),
        };
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// The head of `if` or `for`: a condition, `pattern = value`, or for loops `pattern in value`.
    fn head(&mut self, keyword: TokenKind) {
        let mark = self.out.mark();
        self.expr(Brace::Ends);
        let kind = if self.at(TokenKind::Eq) {
            NodeKind::Test
        } else if keyword == TokenKind::For && self.at_word(Context::ForPattern) {
            NodeKind::In
        } else {
            return;
        };
        let token = self.bump();
        self.expr(Brace::Ends);
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// A block-like expression, possibly under `$` or `unsafe`.
    fn block_like(&mut self, list: List) {
        if matches!(self.peek(), TokenKind::Dollar | TokenKind::Unsafe) {
            let mark = self.out.mark();
            let token = self.bump();
            self.nested(|p| p.block_like(list));
            self.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
        } else {
            self.control(list);
        }
    }

    fn block_like_at(&self) -> bool {
        let mut ahead = 0;
        while matches!(self.nth(ahead), TokenKind::Dollar | TokenKind::Unsafe) {
            ahead += 1;
        }
        matches!(
            self.nth(ahead),
            TokenKind::LBrace | TokenKind::If | TokenKind::For | TokenKind::Match | TokenKind::Label
        )
    }

    fn jump(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let token = self.bump();
        let (kind, operand) = match self.kind(token) {
            TokenKind::Return => (NodeKind::Return, Operand::Optional),
            TokenKind::Break => (NodeKind::Break, Operand::Optional),
            TokenKind::Continue => (NodeKind::Continue, Operand::Never),
            TokenKind::Yield => (NodeKind::Yield, Operand::Required),
            TokenKind::Defer => (NodeKind::Defer, Operand::Required),
            _ => unreachable!("jump called at a jump keyword"),
        };
        if matches!(kind, NodeKind::Break | NodeKind::Continue) && self.at(TokenKind::Label) {
            self.leaf(NodeKind::Label);
        }
        match operand {
            Operand::Required => self.expr(brace),
            Operand::Optional if self.starts_expr() => self.expr(brace),
            Operand::Optional | Operand::Never => {}
        }
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// Runs `body` one level deeper, or skips the operand at the current token when nesting is
    /// too deep.
    fn nested(&mut self, body: impl FnOnce(&mut Self)) {
        if self.depth >= MAX_NESTING {
            let msg = Msg::new().text("nesting is deeper than ").num(MAX_NESTING).text(" levels");
            self.report_here(errors::SYNTAX_NESTING_TOO_DEEP, msg);
            self.skip_group();
            return;
        }
        self.depth += 1;
        body(self);
        self.depth -= 1;
    }

    // Recovery.

    /// Skips tokens as `Error` leaves up to one in `stops`, or a closing delimiter, outside any
    /// group opened while skipping.
    fn recover(&mut self, stops: &[TokenKind]) {
        let mut depth = 0usize;
        loop {
            let k = self.peek();
            if k == TokenKind::Eof || (depth == 0 && (stops.contains(&k) || k.closes())) {
                return;
            }
            if k.opens() {
                depth += 1;
            } else if k.closes() {
                depth -= 1;
            }
            self.leaf(NodeKind::Error);
        }
    }

    /// Skips the current token as an `Error` leaf, with its whole group if it opens one.
    fn skip_group(&mut self) {
        if self.at(TokenKind::Eof) {
            self.missing(NodeKind::Error);
            return;
        }
        let opens = self.peek().opens();
        self.leaf(NodeKind::Error);
        if opens {
            self.recover(&[]);
            if !self.at(TokenKind::Eof) {
                self.leaf(NodeKind::Error);
            }
        }
    }

    // Diagnostics.

    fn report(&mut self, span: Span, d: Diagnostic) {
        // One position reports once, and the lexer already reported its malformed tokens.
        if self.reported == Some(self.pos) || self.tokens.diagnosed(self.current()) {
            return;
        }
        self.reported = Some(self.pos);
        self.cx.error_at(span, d);
    }

    fn report_here(&mut self, id: DiagId, msg: Msg) {
        let span = self.tokens.span(self.current());
        self.report(span, Diagnostic::error(id, msg));
    }

    fn report_expected(&mut self, expected: &[TokenKind]) {
        let texts = expected.iter().map(|k| k.text().unwrap_or_default());
        let msg = self.found(Msg::new().text("expected ").code_list(texts).text(", found "));
        self.report_missing(errors::SYNTAX_EXPECTED_TOKEN, msg);
    }

    /// Reports something absent right after the previous token, where it belongs.
    fn report_missing(&mut self, id: DiagId, msg: Msg) {
        self.report(self.gap(), Diagnostic::error(id, msg));
    }

    /// The current or previous token, if it is a keyword that may have been meant as a name.
    fn nearby_keyword(&self) -> Option<TokenId> {
        let previous = self.pos.checked_sub(1).map(TokenId::new);
        [Some(self.current()), previous]
            .into_iter()
            .flatten()
            .find(|&t| TokenKind::KEYWORDS.contains(&self.kind(t)))
    }

    /// The empty span right after the previous token, where something absent belongs.
    fn gap(&self) -> Span {
        match self.pos.checked_sub(1) {
            Some(previous) => {
                let end = self.tokens.range(TokenId::new(previous)).hi;
                Span { file: self.tokens.file(), range: ByteRange::new(end, end) }
            }
            None => self.tokens.span(self.current()),
        }
    }

    fn report_expected_entry(&mut self, list: List) {
        let (id, what) = match list {
            List::Members => (errors::SYNTAX_EXPECTED_DECLARATION, "expected a declaration, found "),
            List::Statements | List::Items(_) => {
                (errors::SYNTAX_EXPECTED_EXPRESSION, "expected an expression, found ")
            }
        };
        let msg = self.found(Msg::new().text(what));
        self.report_here(id, msg);
    }

    fn report_chained(&mut self) {
        let op = self.text(self.current()).to_owned();
        let msg = Msg::new().code(op).text(" cannot follow an operator of the same precedence");
        let help = Msg::new().text("add parentheses, or join comparisons with ").code("and");
        let d = Diagnostic::error(errors::SYNTAX_CHAINED_OPERATOR, msg).child(ChildKind::Help, help);
        let span = self.tokens.span(self.current());
        self.report(span, d);
    }

    /// Appends the current token to `msg`.
    fn found(&self, msg: Msg) -> Msg {
        match self.peek() {
            TokenKind::Eof => msg.text("end of file"),
            _ => msg.code(self.text(self.current())),
        }
    }

    // Tokens.

    fn current(&self) -> TokenId {
        TokenId::new(self.pos)
    }

    fn nth(&self, ahead: usize) -> TokenKind {
        let last = self.tokens.len() - 1;
        self.tokens.kind(TokenId::new((self.pos + ahead).min(last)))
    }

    fn peek(&self) -> TokenKind {
        self.nth(0)
    }

    fn kind(&self, t: TokenId) -> TokenKind {
        self.tokens.kind(t)
    }

    fn text(&self, t: TokenId) -> &str {
        self.tokens.range(t).of(self.source)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek() == kind
    }

    fn at_word(&self, context: Context) -> bool {
        self.at(TokenKind::Ident) && contextual(self.text(self.current()), context)
    }

    fn starts_expr(&self) -> bool {
        let k = self.peek();
        PREFIX.contains(&k) || OPERANDS.contains(&k)
    }

    /// Consumes the current token, which some node must anchor. After a token the lexer
    /// reported, the next position stays quiet: what follows is usually its fallout.
    fn bump(&mut self) -> TokenId {
        debug_assert!(!self.at(TokenKind::Eof), "Eof anchors the file node only");
        let t = self.current();
        self.pos += 1;
        if self.tokens.diagnosed(t) {
            self.reported = Some(self.pos);
        }
        t
    }

    fn leaf(&mut self, kind: NodeKind) {
        let mark = self.out.mark();
        let token = self.bump();
        self.out.node(kind, Anchor::Token(token), mark);
    }

    fn missing(&mut self, kind: NodeKind) {
        let mark = self.out.mark();
        self.out.node(kind, Anchor::Missing(self.current()), mark);
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        let found = self.at(kind);
        if found {
            self.leaf(NodeKind::Punct);
        }
        found
    }

    fn expect(&mut self, kind: TokenKind) {
        if !self.eat(kind) {
            self.report_expected(&[kind]);
            self.missing(NodeKind::Punct);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cst::NodeId;
    use crate::lexer::lex;
    use overt_diag::DiagCtxt;
    use overt_origin::OriginTable;
    use overt_span::SourceFiles;

    /// The tree as an s-expression: `(Kind'token children)` for branches, the token text for
    /// leaves, `?Kind` for missing nodes.
    fn sexp(cst: &Cst, tokens: &Tokens, text: &str, n: NodeId) -> std::string::String {
        let token = match cst.anchor(n) {
            Anchor::Token(t) => tokens.range(t).of(text).to_owned(),
            Anchor::Missing(_) => return format!("?{:?}", cst.kind(n)),
        };
        if cst.kind(n).is_leaf() {
            return token;
        }
        let children: Vec<_> = cst.children(n).map(|c| sexp(cst, tokens, text, c)).collect();
        let mut out = format!("({:?}'{token}", cst.kind(n));
        for child in children {
            out.push(' ');
            out.push_str(&child);
        }
        out.push(')');
        out
    }

    /// The top-level entries of `text` and the diagnostic ids, after checking the verifier.
    fn parse_text(text: &str) -> (Vec<std::string::String>, Vec<DiagId>) {
        let file = SourceFiles::new().add("t.ov", text);
        let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
        let tokens = lex(file, text, &mut cx);
        let cst = parse(&tokens, text, &mut cx);
        assert_eq!(cst.verify(&tokens), Ok(()), "{text:?}");
        let entries = cst.children(cst.root()).map(|n| sexp(&cst, &tokens, text, n)).collect();
        (entries, diag.sorted().into_iter().map(|d| d.id.clone()).collect())
    }

    fn clean(text: &str) -> Vec<std::string::String> {
        let (entries, ids) = parse_text(text);
        assert!(ids.is_empty(), "unexpected {ids:?} for {text:?}");
        entries
    }

    fn first(text: &str) -> std::string::String {
        clean(text).swap_remove(0)
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(first("x := a + b * c - d;"), "(Binding':= x (Infix'- (Infix'+ a (Infix'* b c)) d))");
        assert_eq!(
            first("x := -a.b.*? or !c and d;"),
            "(Binding':= x (Infix'or (Prefix'- (Postfix'? (Postfix'.* (Member'. a b)))) (Infix'and (Prefix'! c) d)))"
        );
        assert_eq!(first("x := a..=b + 1;"), "(Binding':= x (Infix'..= a (Infix'+ b 1)))");
        assert_eq!(first("x := s[i..];"), "(Binding':= x (Index'[ s (Infix'.. i) ]))");
    }

    #[test]
    fn groups_become_functions_by_what_follows() {
        assert_eq!(
            first("f := (a: u8) -> u8 { a };"),
            "(Binding':= f (Fn'( (Param': a u8) ) (Returns'-> u8) (Block'{ a })))"
        );
        assert_eq!(first("f := () { };"), "(Binding':= f (Fn'( ) (Block'{ })))");
        assert_eq!(
            first("f: (u8, bool) -> u8 = _;"),
            "(Binding': f (FnType'( u8 , bool ) (Returns'-> u8)) = _)"
        );
        assert_eq!(first("t := (a, b);"), "(Binding':= t (Tuple'( a , b )))");
        assert_eq!(first("t := (a,);"), "(Binding':= t (Tuple'( a , )))");
        assert_eq!(first("t := ();"), "(Binding':= t (Tuple'( )))");
        assert_eq!(first("p := (a);"), "(Binding':= p (Paren'( a )))");
        assert_eq!(
            first("P := ($T: type) -> type { struct {} };"),
            "(Binding':= P (Fn'( (Param': (Prefix'$ T) type) ) (Returns'-> type) (Block'{ (Struct'struct { }) })))"
        );
        assert_eq!(
            first("f: unsafe (p: *u8) = _;"),
            "(Binding': f (Prefix'unsafe (FnType'( (Param': p (Prefix'* u8)) ))) = _)"
        );
    }

    #[test]
    fn brackets() {
        assert_eq!(first("a := [1, 2];"), "(Binding':= a (Array'[ 1 , 2 ]))");
        assert_eq!(first("a := [0; 4];"), "(Binding':= a (ArrayRepeat'[ 0 ; 4 ]))");
        assert_eq!(first("mut b: [N]u8;"), "(Binding': mut b (ArrayType'[ N ] u8))");
        assert_eq!(
            first("b: &static [_]u8 = _;"),
            "(Binding': b (Prefix'& static (ArrayType'[ _ ] u8)) = _)"
        );
        assert_eq!(first("m: [2][3]u8 = _;"), "(Binding': m (ArrayType'[ 2 ] (ArrayType'[ 3 ] u8)) = _)");
        assert_eq!(
            first("c := [&mut n] (x: u8) { n += x; };"),
            "(Binding':= c (Closure'[ (Prefix'& mut n) ] (Fn'( (Param': x u8) ) (Block'{ (Assign'+= n x) ; }))))"
        );
        assert_eq!(first("c := [] () {};"), "(Binding':= c (Closure'[ ] (Fn'( ) (Block'{ }))))");
    }

    #[test]
    fn literals() {
        assert_eq!(
            first("p := Point{ .x = 1 };"),
            "(Binding':= p (StructLit'{ Point (FieldInit'= (DotName'. x) 1) }))"
        );
        assert_eq!(
            first("p := .{ .x = 1, 2 };"),
            "(Binding':= p (StructLit'.{ (FieldInit'= (DotName'. x) 1) , 2 }))"
        );
        assert_eq!(first("s := .some(3);"), "(Binding':= s (Call'( (DotName'. some) 3 )))");
        assert_eq!(first("E := enum { \\unsafe, \\x };"), "(Binding':= E (Enum'enum { \\unsafe , \\x }))");
        assert_eq!(first("r := \\\"C:\\dir\";"), "(Binding':= r \\\"C:\\dir\")");
        assert_eq!(first("E := enum { a, b(u8) };"), "(Binding':= E (Enum'enum { a , (Call'( b u8 )) }))");
        assert_eq!(
            first("q := #{ $x * $x };"),
            "(Binding':= q (Quote'#{ (Infix'* (Prefix'$ x) (Prefix'$ x)) }))"
        );
        assert_eq!(first("s := \\\\ a\n    \\\\ b\n;"), "(Binding':= s (MultilineString'\\\\ a \\\\ b))");
    }

    #[test]
    fn heads_end_at_the_body() {
        assert_eq!(
            first("f := () { if a == B { c } else if d { e } };"),
            "(Binding':= f (Fn'( ) (Block'{ (If'if (Infix'== a B) (Block'{ c }) else (If'if d (Block'{ e }))) })))"
        );
        assert_eq!(
            first("f := () { for x in xs { } for .some(j) = q.pop() { } for n < 3 { } for { } };"),
            "(Binding':= f (Fn'( ) (Block'{ (For'for (In'in x xs) (Block'{ })) (For'for (Test'= (Call'( (DotName'. some) j )) (Call'( (Member'. q pop) ))) (Block'{ })) (For'for (Infix'< n 3) (Block'{ })) (For'for (Block'{ })) })))"
        );
        assert_eq!(
            first("f := () { match c { .a(x) if x > 0 => x, _ => {}, } };"),
            "(Binding':= f (Fn'( ) (Block'{ (Match'match c { (Arm'=> (Call'( (DotName'. a) x )) (Guard'if (Infix'> x 0)) x) , (Arm'=> _ (Block'{ })) , }) })))"
        );
    }

    #[test]
    fn statements() {
        assert_eq!(
            first("f := () { if a { b } (c, d) := e; 'l: for { break 'l x; } return; };"),
            "(Binding':= f (Fn'( ) (Block'{ (If'if a (Block'{ b })) (Binding':= (Tuple'( c , d )) e) ; (Labeled''l : (For'for (Block'{ (Break'break 'l x) ; }))) (Return'return) ; })))"
        );
        assert_eq!(
            first("f := () { _ = g(&out b); defer h(); };"),
            "(Binding':= f (Fn'( ) (Block'{ (Assign'= _ (Call'( g (Prefix'& out b) ))) ; (Defer'defer (Call'( h ))) ; })))"
        );
        assert_eq!(
            first("f := () { n := yield ..s.read(); unsafe { g(); } };"),
            "(Binding':= f (Fn'( ) (Block'{ (Binding':= n (Yield'yield (Prefix'.. (Call'( (Member'. s read) ))))) ; (Prefix'unsafe (Block'{ (Call'( g )) ; })) })))"
        );
    }

    #[test]
    fn members_fields_and_annotations() {
        assert_eq!(
            first("S := struct { x: u8, y: u8 = 0, d := 1; };"),
            "(Binding':= S (Struct'struct { (Field': x u8) , (Field': y u8 = 0) , (Binding':= d 1) ; }))"
        );
        assert_eq!(
            first("S := struct { $for r in rs { #[a(1), b] $(r.name): u8, } };"),
            "(Binding':= S (Struct'struct { (Prefix'$ (For'for (In'in r rs) (Block'{ (Attr'#[ (Call'( a 1 )) , b ] (Field': (Prefix'$ (Paren'( (Member'. r name) ))) u8)) , }))) }))"
        );
        let entries = clean("//! m\n/// d\n#[t] x := 1;");
        assert_eq!(entries, ["//! m", "(Doc'/// d (Attr'#[ t ] (Binding':= x 1)))", ";"]);
    }

    #[test]
    fn errors_are_reported_once_and_the_tree_verifies() {
        let cases: &[(&str, &[DiagId])] = &[
            ("x := 1\ny := 2;", &[errors::SYNTAX_EXPECTED_TOKEN]),
            ("x := ;", &[errors::SYNTAX_EXPECTED_EXPRESSION]),
            ("x := f(a, b;", &[errors::SYNTAX_UNCLOSED_DELIMITER]),
            ("x := f(a b);", &[errors::SYNTAX_EXPECTED_TOKEN]),
            ("x := a < b < c;", &[errors::SYNTAX_CHAINED_OPERATOR]),
            ("x := a.;", &[errors::SYNTAX_EXPECTED_NAME]),
            ("f(x);", &[errors::SYNTAX_EXPECTED_TOKEN]),
            (") x := 1;", &[errors::SYNTAX_EXPECTED_DECLARATION]),
            ("x := { a b };", &[errors::SYNTAX_EXPECTED_TOKEN]),
            ("x := @;", &[errors::SYNTAX_UNKNOWN_CHARACTER]),
            ("x := match a { .b c, };", &[errors::SYNTAX_EXPECTED_TOKEN]),
            ("x := struct { a b };", &[errors::SYNTAX_EXPECTED_TOKEN]),
            ("x := (", &[errors::SYNTAX_UNCLOSED_DELIMITER]),
            ("/// dangling", &[errors::SYNTAX_EXPECTED_DECLARATION]),
        ];
        for &(text, expected) in cases {
            assert_eq!(parse_text(text).1, expected, "{text:?}");
        }
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let depth = MAX_NESTING as usize * 2;
        let text = format!("x := {}1{};", "(".repeat(depth), ")".repeat(depth));
        assert_eq!(parse_text(&text).1, [errors::SYNTAX_NESTING_TOO_DEEP]);
        let text = format!("x := {}1;", "-".repeat(depth));
        assert_eq!(parse_text(&text).1, [errors::SYNTAX_NESTING_TOO_DEEP]);
    }
}
