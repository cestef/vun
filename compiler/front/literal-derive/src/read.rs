//! `FromLiteral`: each tagged form tried by its name, then each untagged one in order, each
//! failure naming what was expected where.

use proc_macro::{Delimiter, TokenStream};
use vun_derive_core::Tokens;

use crate::options::{Form, Literal, Shape};

/// A path in `vun_syntax::literal`.
fn literal(name: &str) -> Tokens {
    Tokens::new().path(&["vun_syntax", "literal", name])
}

fn value() -> Tokens {
    Tokens::new().word("value")
}

/// `value.method(arguments)?`
fn read(method: &str, arguments: Tokens) -> Tokens {
    value().op(".").word(method).parens(arguments).op("?")
}

impl Literal<'_> {
    pub(crate) fn read(&self) -> TokenStream {
        let body = match self.shapes.as_slice() {
            [one] if matches!(one.form, Form::Record | Form::Newtype) => one.read(),
            shapes => {
                let tagged = shapes.iter().filter(|s| s.form != Form::Untagged);
                let untagged = shapes.iter().filter(|s| s.form == Form::Untagged);
                let tries = tagged.chain(untagged).fold(Tokens::new(), |t, s| t.then(s.read()));
                let expected = if shapes.iter().any(|s| s.form == Form::Untagged) {
                    literal("Expected").op("::").word("Form")
                } else {
                    let names = shapes.iter().map(|s| Tokens::new().string(&s.name));
                    let names = Tokens::new()
                        .op("&")
                        .group(Delimiter::Bracket, Tokens::new().comma(names, Tokens::then));
                    literal("Expected").op("::").word("OneOf").parens(names)
                };
                tries.err(literal("Mismatch").op("::").word("new").parens(expected))
            }
        };
        let method = Tokens::new()
            .word("fn")
            .word("from_literal")
            .parens(value().op(":").op("&").then(literal("Literal")).op("<").lifetime("_").op(">"))
            .op("->")
            .result(literal("Mismatch"))
            .braces(body);
        self.item.implement(&literal("FromLiteral"), method)
    }
}

impl Shape<'_> {
    /// For a struct, its value read; for a variant, a statement returning it if the value has its
    /// form.
    fn read(&self) -> Tokens {
        let path = || self.case.path.clone();
        match self.form {
            Form::Record => {
                let fields = self.fields.iter().map(|(_, name)| read("field", Tokens::new().string(name)));
                Tokens::new().ok(self.case.construct(fields))
            }
            Form::Newtype => Tokens::new().ok(path().parens(read("read", Tokens::new()))),
            Form::Name => Tokens::new()
                .word("if")
                .then(value())
                .op(".")
                .word("is_name")
                .parens(Tokens::new().string(&self.name))
                .braces(Tokens::new().word("return").ok(path()).op(";")),
            Form::Tagged { rest } => {
                let count = self.fields.len();
                let args = Tokens::new().word("args");
                let values = (0..count).map(|i| {
                    let method = if rest && i + 1 == count { "from" } else { "at" };
                    args.clone().op(".").word(method).parens(Tokens::new().count(i)).op("?")
                });
                let arguments = Tokens::new().string(&self.name).op(",").count(count).op(",").word(if rest {
                    "true"
                } else {
                    "false"
                });
                let some = Tokens::new().path(&["core", "option", "Option", "Some"]).parens(args.clone());
                Tokens::new()
                    .word("if")
                    .word("let")
                    .then(some)
                    .op("=")
                    .then(read("arguments", arguments))
                    .braces(Tokens::new().word("return").ok(self.case.construct(values)).op(";"))
            }
            Form::Untagged => {
                let fits = Tokens::new().word("fits");
                let ok = Tokens::new().path(&["core", "result", "Result", "Ok"]).parens(fits.clone());
                let attempt = literal("FromLiteral")
                    .op("::")
                    .word("from_literal")
                    .parens(value())
                    .op(".")
                    .word("map")
                    .parens(path());
                Tokens::new()
                    .word("if")
                    .word("let")
                    .then(ok)
                    .op("=")
                    .then(attempt)
                    .braces(Tokens::new().word("return").ok(fits).op(";"))
            }
        }
    }
}
