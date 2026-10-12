//! A C enum as a Vun enum, when the request says C holds no other value in it: each constant a
//! variant naming its tag, with the prefix the constants share dropped (`GTK_ALIGN_FILL` is
//! `fill`), and a constant that repeats a value another has a constant of the enum's type.

use std::collections::HashMap;

use vun_clang::{Cursor, Kind};
use vun_syntax::case::Case;
use vun_syntax::lexer::is_ident;
use vun_syntax::make::{Expr, Item, Variant};

use crate::emit::item;
use crate::{Binder, items};

/// What separates the words of a C constant's name, where a shared prefix ends.
const WORD_END: char = '_';

impl Binder<'_> {
    /// `decl` as a Vun enum whose tag is the integer `tag`, with a constant for each of its names
    /// that repeats a value; `None` when it is not asked for, or its names make no variants: two
    /// that are one name once the prefix is dropped, or one that is no name.
    pub(crate) fn closed(&self, decl: &Cursor<'_>, tag: &Expr) -> Option<Vec<Item>> {
        let name = self.bound(decl)?;
        if !self.closed.iter().any(|glob| glob.matches(&name)) {
            return None;
        }
        let constants: Vec<Cursor<'_>> =
            decl.children().into_iter().filter(|c| c.kind() == Kind::EnumConstant).collect();
        let prefix = Self::shared_prefix(&constants);
        let mut variants: Vec<Variant> = Vec::new();
        let mut by_value: HashMap<i64, String> = HashMap::new();
        let mut repeats: Vec<(String, String)> = Vec::new();
        for constant in &constants {
            let written = constant.name();
            let word = Case::Snake.write(written.strip_prefix(prefix.as_str()).unwrap_or(&written));
            if !is_ident(&word) {
                return None;
            }
            match by_value.get(&constant.enum_value()) {
                Some(first) => repeats.push((written, first.clone())),
                None if variants.iter().any(|v| v.name == word) => return None,
                None => {
                    by_value.insert(constant.enum_value(), word.clone());
                    variants.push(Variant::coded(&word, Expr::int(i128::from(constant.enum_value()))));
                }
            }
        }
        let doc = vec!["Its variants name the tags C gives them.".to_owned()];
        let attrs = vec![items::layout_tag(tag.clone())];
        let mut made = vec![Item { doc, attrs, ..item(&name, Expr::Enum(variants, Vec::new())) }];
        for (written, first) in repeats {
            let value = Expr::DotName(first);
            made.push(item(&self.vun_name(&written, Case::Snake), value).typed(Expr::local(&name)));
        }
        Some(made)
    }

    /// The words every constant starts with, the last `_` included: nothing for one constant,
    /// which has no other to share with.
    fn shared_prefix(constants: &[Cursor<'_>]) -> String {
        let names: Vec<String> = constants.iter().map(Cursor::name).collect();
        let [first, rest @ ..] = names.as_slice() else { return String::new() };
        if rest.is_empty() {
            return String::new();
        }
        let shared = rest.iter().fold(first.len(), |len, name| {
            first.bytes().zip(name.bytes()).take(len).take_while(|(a, b)| a == b).count()
        });
        match first[..shared].rfind(WORD_END) {
            Some(end) => first[..=end].to_owned(),
            None => String::new(),
        }
    }
}
