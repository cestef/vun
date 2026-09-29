//! `ToJson`: each case of the value matched, and written in the item's form.

use proc_macro::TokenStream;
use vun_derive_core::{Fields, Tokens};

use crate::options::{Form, Json, Shape};
use crate::tokens::JsonTokens;

impl Json<'_> {
    pub(crate) fn write(&self) -> TokenStream {
        let arms = self.shapes.iter().fold(Tokens::new(), |arms, shape| {
            arms.then(shape.case.pattern()).op("=>").braces(self.written(shape))
        });
        // An enum without variants has no value to match a reference to.
        let scrutinee = if self.shapes.is_empty() { Tokens::new().op("*") } else { Tokens::new() };
        let method = Tokens::new()
            .word("fn")
            .word("to_json")
            .parens(Tokens::new().op("&").word("self"))
            .op("->")
            .json("Value")
            .op("<")
            .lifetime("static")
            .op(">")
            .braces(Tokens::new().word("match").then(scrutinee).word("self").braces(arms));
        self.item.implement(&Tokens::new().json("ToJson"), method)
    }

    fn written(&self, shape: &Shape<'_>) -> Tokens {
        let from = |value: Tokens| Tokens::new().value("from").parens(value);
        match self.form {
            Form::Names => from(Tokens::new().string(&shape.name)),
            Form::Numbers => from(shape.case.path.clone().word("as").word("i64")),
            Form::Struct | Form::Untagged => shape.written(),
        }
    }
}

impl Shape<'_> {
    /// Its fields written: an object, the one field, an array, or `null`.
    fn written(&self) -> Tokens {
        let to_json = |binding| Tokens::new().call("ToJson", "to_json", Tokens::new().tree(binding));
        match self.case.fields {
            Fields::Named(_) => {
                let object = Tokens::new().word("object");
                let members = self.members.iter().fold(Tokens::new(), |members, m| {
                    let binding = m.field.binding.clone();
                    let written = if m.flatten {
                        Tokens::new().word("merge").parens(to_json(binding))
                    } else {
                        Tokens::new()
                            .word("write")
                            .parens(Tokens::new().string(&m.name).op(",").tree(binding))
                    };
                    members.then(object.clone()).op(".").then(written).op(";")
                });
                Tokens::new()
                    .word("let")
                    .word("mut")
                    .then(object.clone())
                    .op("=")
                    .json("Object")
                    .op("::")
                    .word("new")
                    .parens(Tokens::new())
                    .op(";")
                    .then(members)
                    .value("Object")
                    .parens(object)
            }
            Fields::Tuple(fields) if fields.len() == 1 => to_json(fields[0].binding.clone()),
            Fields::Tuple(fields) => {
                let items = Tokens::new().comma(fields, |t, f| t.then(to_json(f.binding.clone())));
                let items = Tokens::new().group(proc_macro::Delimiter::Bracket, items);
                let vec = Tokens::new().path(&["std", "vec", "Vec", "from"]).parens(items);
                Tokens::new().value("Array").parens(vec)
            }
            Fields::Unit => Tokens::new().value("Null"),
        }
    }
}
