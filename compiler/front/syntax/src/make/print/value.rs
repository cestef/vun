//! Every form, by what it spells: literals, aggregates, types and functions, and the dispatch to
//! the operators and the control forms.

use std::fmt::Write as _;

use vun_grammar::{Level, TokenKind};

use super::Place;
use crate::lexer::{C_STRING, ESCAPE, SIMPLE_ESCAPES};
use crate::literal::Writer;
use crate::make::{Expr, Function, Init, Param};

impl Writer {
    /// `expr`, whose lines after the first are indented as `at` says. It is not parenthesized:
    /// `operand` does that where a level asks.
    pub(super) fn expr(&mut self, expr: &Expr, at: Place) {
        match expr {
            Expr::Int(n) => {
                let _ = write!(self.out, "{n}");
            }
            Expr::Float(bits) => self.float(f64::from_bits(*bits)),
            Expr::Char(c) => self.character(*c),
            Expr::String(s) => self.string(s),
            Expr::CString(s) => {
                self.text(&C_STRING.to_string());
                self.string(s);
            }
            Expr::Hole => {
                self.token(TokenKind::Underscore);
            }
            Expr::Unit => {
                self.token(TokenKind::LParen).token(TokenKind::RParen);
            }
            Expr::Path(path) => {
                for (i, name) in path.iter().enumerate() {
                    if i > 0 {
                        self.token(TokenKind::Dot);
                    }
                    self.binder(name).expect("made names are identifiers");
                }
            }
            Expr::Tuple(items) => {
                self.token(TokenKind::LParen).list(items, at);
                if items.len() == 1 {
                    self.token(TokenKind::Comma);
                }
                self.token(TokenKind::RParen);
            }
            Expr::List(items) => {
                self.token(TokenKind::LBracket).list(items, at);
                self.token(TokenKind::RBracket);
            }
            Expr::Repeat(value, count) => self.repeat(value, count, at),
            Expr::Record(inits) => {
                self.token(TokenKind::DotLBrace);
                self.inits(inits, at);
            }
            Expr::Typed(ty, inits) => {
                self.operand(ty, Expr::postfix_level(), at);
                self.token(TokenKind::LBrace);
                self.inits(inits, at);
            }
            Expr::Block(block) => self.block(block, at),
            Expr::ArrayType(len, item) => self.array_type(len, item, at),
            Expr::FnType(params, result) => self.signature(params, result.as_deref(), at),
            Expr::Struct(members) => self.struct_type(members, at),
            Expr::Enum(variants, members) => self.enum_type(variants, members, at),
            Expr::Function(function) => self.function(function, at),
            Expr::Closure(captures, function) => {
                self.token(TokenKind::LBracket).list(captures, at);
                self.token(TokenKind::RBracket).text(" ");
                self.function(function, at);
            }
            Expr::Member(object, property) => self.member(object, property, at),
            Expr::DotName(name) => {
                self.token(TokenKind::Dot).binder(name).expect("made names are identifiers");
            }
            Expr::Call(callee, args) => self.call(callee, args, at),
            Expr::Index(object, subscript) => self.index(object, subscript, at),
            Expr::Prefix(token, inner) => self.prefix(*token, inner, at),
            Expr::Postfix(token, inner) => self.postfix(*token, inner, at),
            Expr::Infix(token, left, right) => self.infix(*token, left, right.as_deref(), at),
            Expr::Borrow(borrow, inner) => self.borrow(*borrow, inner, at),
            Expr::Assign(token, place, value) => self.assign(*token, place, value, at),
            Expr::Bind(bind) => self.bind(bind, at),
            Expr::If(condition, then, otherwise) => self.if_form(condition, then, otherwise.as_deref(), at),
            Expr::Test(pattern, value) => {
                self.head(pattern, value, |w| w.text(" ").token(TokenKind::Eq).text(" "), at);
            }
            Expr::For(head, body) => self.for_form(head.as_deref(), body, at),
            Expr::In(pattern, value) => self.head(pattern, value, |w| w.text(" in "), at),
            Expr::Match(scrutinee, arms) => self.match_form(scrutinee, arms, at),
            Expr::Labeled(label, body) => self.labeled(label, body, at),
            Expr::Return(value) => self.jump(TokenKind::Return, None, value.as_deref(), at),
            Expr::Break(label, value) => self.jump(TokenKind::Break, label.as_deref(), value.as_deref(), at),
            Expr::Continue(label) => self.jump(TokenKind::Continue, label.as_deref(), None, at),
            Expr::Yield(value) => self.jump(TokenKind::Yield, None, Some(value), at),
            Expr::Defer(body) => self.jump(TokenKind::Defer, None, Some(body), at),
            Expr::Attributed(attrs, inner) => {
                self.attributes(attrs, at).text(" ");
                self.operand(inner, Level::LOWEST, at);
            }
            Expr::Staged(inner) => {
                self.token(TokenKind::Dollar);
                self.expr(inner, at);
            }
        }
    }

    /// `[value; count]`
    fn repeat(&mut self, value: &Expr, count: &Expr, at: Place) {
        self.token(TokenKind::LBracket);
        self.operand(value, Level::LOWEST, at.within());
        self.token(TokenKind::Semi).text(" ");
        self.operand(count, Level::LOWEST, at.within());
        self.token(TokenKind::RBracket);
    }

    /// `[len]item`
    fn array_type(&mut self, len: &Expr, item: &Expr, at: Place) {
        self.token(TokenKind::LBracket);
        self.operand(len, Level::LOWEST, at.within());
        self.token(TokenKind::RBracket);
        self.operand(item, Expr::prefix_level(), at);
    }

    /// `pattern <joint> value`, a head matching or iterating.
    fn head(&mut self, pattern: &Expr, value: &Expr, joint: impl FnOnce(&mut Self) -> &mut Self, at: Place) {
        self.operand(pattern, Expr::value_level(), at.within());
        joint(self);
        self.operand(value, Expr::value_level(), at);
    }

    /// `{ a, .b = c }`, after its opening token.
    fn inits(&mut self, inits: &[Init], at: Place) {
        if !inits.is_empty() {
            self.text(" ");
            for (i, init) in inits.iter().enumerate() {
                if i > 0 {
                    self.token(TokenKind::Comma).text(" ");
                }
                match init {
                    Init::Item(value) => {
                        self.operand(value, Level::LOWEST, at.within());
                    }
                    Init::Named(name, value) => {
                        self.token(TokenKind::Dot).binder(name).expect("made names are identifiers");
                        self.text(" ").token(TokenKind::Eq).text(" ");
                        self.operand(value, Expr::value_level(), at.within());
                    }
                }
            }
            self.text(" ");
        }
        self.token(TokenKind::RBrace);
    }

    /// `(params) -> result`
    fn signature(&mut self, params: &[Param], result: Option<&Expr>, at: Place) {
        self.token(TokenKind::LParen);
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.token(TokenKind::Comma).text(" ");
            }
            if let Some(pattern) = &param.pattern {
                self.operand(pattern, Expr::value_level(), at.within());
                self.token(TokenKind::Colon).text(" ");
            }
            self.operand(&param.ty, Expr::value_level(), at.within());
        }
        self.token(TokenKind::RParen);
        if let Some(result) = result {
            self.text(" ").token(TokenKind::Arrow).text(" ");
            self.operand(result, Expr::value_level(), at.heading());
        }
    }

    /// `(params) -> result { body }`
    fn function(&mut self, function: &Function, at: Place) {
        self.signature(&function.params, function.result.as_deref(), at);
        self.text(" ");
        self.block(&function.body, at);
    }

    /// A float as the lexer reads one: with a point or an exponent, never `inf` or `NaN`.
    fn float(&mut self, value: f64) {
        assert!(value.is_finite(), "a float literal is finite");
        let _ = write!(self.out, "{value:?}");
    }

    /// `'c'`, escaped as a character literal reads.
    fn character(&mut self, c: char) {
        self.out.push('\'');
        match SIMPLE_ESCAPES.iter().find(|&&(_, value)| value == c && c != '"') {
            Some(&(escape, _)) => {
                self.out.push(ESCAPE);
                self.out.push(escape);
            }
            None if c.is_control() => {
                let _ = write!(self.out, "{ESCAPE}u{{{:x}}}", u32::from(c));
            }
            None => self.out.push(c),
        }
        self.out.push('\'');
    }
}
