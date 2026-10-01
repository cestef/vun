//! The Sublime syntax of Vun (which syntect, and so the whitepaper's Typst, reads), from the
//! grammar: keywords, contextual words, borrow and flow words and every punctuation token, each
//! scoped as `editors.vun` says; the tokens the lexer reads by hand come from its constants
//! (`lexical`).

use crate::generate::{Data, Emit, Format, Generator, Read, Source};

mod lexical;
mod rules;
mod syntax;

use super::SYNTAX;
use super::model::{Editors, Model};
use syntax::{Context, Rule};
pub(crate) use syntax::{Scope, Syntax};

pub(crate) const SUBLIME: Generator =
    Generator { source: SYNTAX, output: "editors/sublime", emit: Emit::Data(emit), keeps: &[] };

const FILE: &str = "vun.sublime-syntax";
const LANGUAGE: &str = "Vun";

/// Where a contextual keyword is recognized: the variants of the grammar's `Context`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_syntax::literal::FromLiteral)]
pub(crate) enum Recognized {
    ForPattern,
    Declaration,
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    Ok(vec![Data { name: FILE.to_owned(), format: Format::Yaml, text: syntax(sources)?.text() }])
}

/// The lexical syntax of Vun, which Sublime syntaxes and `TextMate` grammars write.
pub(crate) fn syntax(sources: &[Source<'_>]) -> Read<Syntax> {
    let model = Model::read(sources)?;
    let editors = Editors::read(&model, sources)?;
    let scoped = Scoped { model: &model, editors: &editors };
    let syntax = Syntax {
        name: LANGUAGE,
        extensions: vec![vun_span::EXTENSION],
        contexts: [
            vec![(Context::Main, main())],
            vec![(Context::Comments, lexical::comments())],
            lexical::strings(),
            vec![
                (Context::Literals, lexical::literals()),
                (Context::Keywords, scoped.keywords()),
                (Context::Names, scoped.names()),
                (Context::Punctuation, scoped.punctuation()),
            ],
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    Ok(syntax)
}

/// A file's contexts, tried in this order.
fn main() -> Vec<Rule> {
    [
        Context::Comments,
        Context::Strings,
        Context::Literals,
        Context::Keywords,
        Context::Names,
        Context::Punctuation,
    ]
    .map(Rule::Include)
    .into()
}

/// The grammar and what `editors.vun` says of the Sublime syntax.
struct Scoped<'m> {
    model: &'m Model,
    editors: &'m Editors,
}
