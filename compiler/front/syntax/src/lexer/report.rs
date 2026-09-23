//! Lexer diagnostics.

use overt_diag::{ChildKind, DiagId, Diagnostic, Msg, errors};

use super::{Lexer, SIMPLE_ESCAPES};

impl Lexer<'_, '_, '_> {
    pub(super) fn unterminated(&mut self, lo: usize, id: DiagId, what: &'static str, close: char) {
        let msg = Msg::new().text("unterminated ").text(what).text(" literal");
        let help = Msg::new().text("add a closing ").code(close);
        let span = self.span_from(lo);
        self.cx.error_at(span, Diagnostic::error(id, msg).child(ChildKind::Help, help));
    }

    pub(super) fn invalid_escape(&mut self, lo: usize) {
        let msg = Msg::new().text("invalid escape sequence ").code(&self.text[lo..self.pos]);
        let simple = SIMPLE_ESCAPES.iter().map(|&(c, _)| ['\\', c].iter().collect::<String>());
        let valid = simple.chain(["\\xHH".to_owned(), "\\u{H..HHHHHH}".to_owned()]);
        let note = Msg::new().text("valid escapes are ").code_list(valid);
        let span = self.span_from(lo);
        let d = Diagnostic::error(errors::SYNTAX_INVALID_ESCAPE, msg).child(ChildKind::Note, note);
        self.cx.error_at(span, d);
    }

    pub(super) fn error(&mut self, lo: usize, id: DiagId, msg: Msg) {
        let span = self.span_from(lo);
        self.cx.error_at(span, Diagnostic::error(id, msg));
    }
}
