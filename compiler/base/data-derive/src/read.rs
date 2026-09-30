//! `FromData`: the value read in the item's form, each failure naming what was expected where.

use proc_macro::{Delimiter, TokenStream};
use vun_derive_core::{Fields, Tokens};

use crate::options::{Data, Form, Shape};
use crate::tokens::JsonTokens;

impl Data<'_> {
    pub(crate) fn read(&self) -> TokenStream {
        let value = || Tokens::new().word("value");
        let body = match self.form {
            Form::Struct => self.shapes[0].read(),
            Form::Names => {
                let arms = self.shapes.iter().fold(Tokens::new(), |arms, shape| {
                    let name = Tokens::new().string(&shape.name);
                    let some = Tokens::new().path(&["core", "option", "Option", "Some"]).parens(name);
                    arms.then(some).op("=>").ok(shape.case.path.clone()).op(",")
                });
                let names = self.shapes.iter().map(|s| Tokens::new().string(&s.name));
                let names =
                    Tokens::new().op("&").group(Delimiter::Bracket, Tokens::new().comma(names, Tokens::then));
                let expected = Tokens::new().expected("OneOf").parens(names);
                let other = Tokens::new().word("_").op("=>").err(Tokens::new().mismatch(expected));
                Tokens::new()
                    .word("match")
                    .then(value())
                    .op(".")
                    .word("as_str")
                    .parens(Tokens::new())
                    .braces(arms.then(other))
            }
            Form::Numbers => {
                let number = Tokens::new().word("number");
                let tests = self.shapes.iter().fold(Tokens::new(), |tests, shape| {
                    let discriminant = shape.case.path.clone().word("as").word("i64");
                    let some = Tokens::new().path(&["core", "option", "Option", "Some"]).parens(discriminant);
                    let found = Tokens::new().word("return").ok(shape.case.path.clone()).op(";");
                    tests.word("if").then(number.clone()).op("==").then(some).braces(found)
                });
                Tokens::new()
                    .word("let")
                    .then(number)
                    .op("=")
                    .then(value())
                    .op(".")
                    .word("as_i64")
                    .parens(Tokens::new())
                    .op(";")
                    .then(tests)
                    .err(Tokens::new().mismatch(Tokens::new().expected("Form")))
            }
            Form::Tagged => self.tagged(),
            Form::Untagged => {
                // Each variant is tried in order, in a closure so that its first failure ends it.
                let tries = self.shapes.iter().fold(Tokens::new(), |tries, shape| {
                    let attempt = Tokens::new()
                        .op("||")
                        .op("->")
                        .result(Tokens::new().data("Mismatch"))
                        .braces(shape.read());
                    let fits = Tokens::new()
                        .path(&["core", "result", "Result", "Ok"])
                        .parens(Tokens::new().word("fits"));
                    tries
                        .word("if")
                        .word("let")
                        .then(fits)
                        .op("=")
                        .parens(attempt)
                        .parens(Tokens::new())
                        .braces(Tokens::new().word("return").ok(Tokens::new().word("fits")).op(";"))
                });
                tries.err(Tokens::new().mismatch(Tokens::new().expected("Form")))
            }
        };
        let method = Tokens::new()
            .word("fn")
            .word("from_data")
            .parens(value().op(":").op("&").data("Value").op("<").lifetime("_").op(">"))
            .op("->")
            .result(Tokens::new().data("Mismatch"))
            .braces(body);
        self.item.implement(&Tokens::new().data("FromData"), method)
    }

    /// A tagged enum read: a string naming a unit variant, or an object whose one member names
    /// another and holds its value.
    fn tagged(&self) -> Tokens {
        let value = || Tokens::new().word("value");
        let unit = |shape: &&Shape<'_>| matches!(shape.case.fields, Fields::Unit);
        let named = self.shapes.iter().filter(unit).fold(Tokens::new(), |arms, shape| {
            let name = Tokens::new().string(&shape.name);
            let some = Tokens::new().path(&["core", "option", "Option", "Some"]).parens(name);
            arms.then(some).op("=>").word("return").ok(shape.case.path.clone()).op(",")
        });
        let other = Tokens::new().word("_").op("=>").braces(Tokens::new());
        let by_name = Tokens::new()
            .word("match")
            .then(value())
            .op(".")
            .word("as_str")
            .parens(Tokens::new())
            .braces(named.then(other.clone()));
        // A variant's value is read in a closure so that its first failure ends it.
        let tagged = self.shapes.iter().filter(|s| !unit(s)).fold(Tokens::new(), |arms, shape| {
            let attempt =
                Tokens::new().op("||").op("->").result(Tokens::new().data("Mismatch")).braces(shape.read());
            let step =
                Tokens::new().data("Step").op("::").word("Member").parens(Tokens::new().string(&shape.name));
            let within =
                Tokens::new().op("|").word("m").op("|").word("m").op(".").word("within").parens(step);
            arms.string(&shape.name)
                .op("=>")
                .word("return")
                .parens(attempt)
                .parens(Tokens::new())
                .op(".")
                .word("map_err")
                .parens(within)
                .op(",")
        });
        let one = Tokens::new()
            .op("|")
            .word("o")
            .op("|")
            .word("o")
            .op(".")
            .word("len")
            .parens(Tokens::new())
            .op("==")
            .count(1);
        let first = Tokens::new()
            .op("|")
            .word("o")
            .op("|")
            .word("o")
            .op(".")
            .word("iter")
            .parens(Tokens::new())
            .op(".")
            .word("next")
            .parens(Tokens::new());
        let member = Tokens::new()
            .path(&["core", "option", "Option", "Some"])
            .parens(Tokens::new().parens(Tokens::new().word("name").op(",").word("value")));
        let found = value()
            .op(".")
            .word("as_object")
            .parens(Tokens::new())
            .op(".")
            .word("filter")
            .parens(one)
            .op(".")
            .word("and_then")
            .parens(first);
        let members = Tokens::new()
            .word("if")
            .word("let")
            .then(member)
            .op("=")
            .then(found)
            .braces(Tokens::new().word("match").word("name").braces(tagged.then(other)));
        by_name.then(members).err(Tokens::new().mismatch(Tokens::new().expected("Form")))
    }
}

impl Shape<'_> {
    /// Statements ending in its value read from `value`, or the first failure returned.
    fn read(&self) -> Tokens {
        let value = || Tokens::new().word("value");
        let from_data = |argument| Tokens::new().call("FromData", "from_data", argument).op("?");
        match self.case.fields {
            Fields::Named(_) => {
                let object = Tokens::new().word("object");
                let members = self.members.iter().map(|m| {
                    if m.flatten {
                        from_data(value())
                    } else {
                        object.clone().op(".").word("read").parens(Tokens::new().string(&m.name)).op("?")
                    }
                });
                let as_object = value().op(".").word("as_object").parens(Tokens::new());
                let missing = Tokens::new().mismatch(Tokens::new().expected("Object"));
                Tokens::new()
                    .word("let")
                    .then(object.clone())
                    .op("=")
                    .then(as_object)
                    .op(".")
                    .word("ok_or")
                    .parens(missing)
                    .op("?")
                    .op(";")
                    .ok(self.case.construct(members))
            }
            Fields::Tuple(fields) if fields.len() == 1 => {
                Tokens::new().ok(self.case.construct([from_data(value())]))
            }
            Fields::Tuple(fields) => {
                let items = Tokens::new().word("items");
                let count = fields.len();
                let length =
                    Tokens::new().word("i").op(".").word("len").parens(Tokens::new()).op("==").count(count);
                let as_array = value()
                    .op(".")
                    .word("as_array")
                    .parens(Tokens::new())
                    .op(".")
                    .word("filter")
                    .parens(Tokens::new().op("|").word("i").op("|").then(length));
                let missing = Tokens::new()
                    .mismatch(Tokens::new().expected("Items").parens(Tokens::new().count(count)));
                let values = (0..count).map(|i| {
                    let item = Tokens::new()
                        .op("&")
                        .then(items.clone())
                        .group(Delimiter::Bracket, Tokens::new().count(i));
                    let step =
                        Tokens::new().data("Step").op("::").word("Item").parens(Tokens::new().count(i));
                    let within =
                        Tokens::new().op("|").word("m").op("|").word("m").op(".").word("within").parens(step);
                    Tokens::new()
                        .call("FromData", "from_data", item)
                        .op(".")
                        .word("map_err")
                        .parens(within)
                        .op("?")
                });
                Tokens::new()
                    .word("let")
                    .then(items.clone())
                    .op("=")
                    .then(as_array)
                    .op(".")
                    .word("ok_or")
                    .parens(missing)
                    .op("?")
                    .op(";")
                    .ok(self.case.construct(values))
            }
            Fields::Unit => {
                let unit = Tokens::new().op("<").parens(Tokens::new()).word("as").data("FromData").op(">");
                unit.op("::").word("from_data").parens(value()).op("?").op(";").ok(self.case.path.clone())
            }
        }
    }
}
