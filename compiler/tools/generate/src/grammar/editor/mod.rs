//! The Sublime syntax of Vun (which syntect, and so the whitepaper's Typst, reads), from the
//! grammar's tables: keywords, contextual words, borrow and flow words and every punctuation
//! token come from `grammar.vun`, each punctuation token scoped by the tables it is in; the
//! tokens the lexer reads by hand come from its constants (`lexical`).

use vun_diag::{DiagId, Msg};
use vun_syntax::literal::Literal;

use super::{Enum, Grammar, SYNTAX, grammar};
use crate::generate::{Data, Emit, Format, Generator, Read, Source};

mod lexical;
mod pattern;
mod rules;
mod syntax;
#[cfg(test)]
mod tests;

use lexical::{IDENT, TYPE_NAME};
use pattern::Pattern;
use syntax::{Context, Rule, Scope, Syntax};

pub(crate) const EDITOR: Generator =
    Generator { source: SYNTAX, output: "editors/sublime", emit: Emit::Data(emit) };

const FILE: &str = "vun.sublime-syntax";
const LANGUAGE: &str = "Vun";

const MISSING: DiagId = DiagId::new("generate.grammar.missing");

/// The kinds and lists the syntax reads, by their names in `grammar.vun`.
const TOKENS: &str = "TokenKind";
const KEYWORDS: &str = "keywords";
const PUNCTUATION: &str = "punctuation";
const BORROWS: &str = "BorrowWord";
const QUALIFIERS: &str = "qualifiers";
const MODES: &str = "modes";
const FLOWS: (&str, &str) = ("Flow", "flows");
const CONTEXTUAL: &str = "contextual";
const DELIMITERS: &str = "delimiters";

/// Tables whose punctuation takes a scope; a token in several takes the first's.
const TABLES: [(&str, Scope); 6] = [
    ("assign", Scope::Assignment),
    ("infix", Scope::Operator),
    ("prefix", Scope::Operator),
    ("postfix", Scope::Operator),
    (DELIMITERS, Scope::Section),
    ("separators", Scope::Separator),
];

/// Punctuation whose scope says more than its tables do.
const SPECIAL: [(&str, Scope); 7] = [
    ("colon_eq", Scope::Declaration),
    ("arrow", Scope::Declaration),
    ("fat_arrow", Scope::Declaration),
    ("dollar", Scope::Stage),
    ("hash_l_brace", Scope::Quote),
    ("hash_l_bracket", Scope::Annotation),
    ("underscore", Scope::Hole),
];

/// Tokens that place the rules for names: a member after `.`, a call before `(`, a binding
/// before `:=`, a borrow after `&`, a block after `{`.
const DOT: &str = "dot";
const CALL: &str = "l_paren";
const BINDS: &str = "colon_eq";
const BORROW: &str = "amp";
const BLOCK: &str = "l_brace";

/// Where a contextual keyword is recognized: the variants of the grammar's `Context`.
#[derive(Clone, Copy)]
enum Recognized {
    ForPattern,
    AsmBlock,
}

impl Recognized {
    const ALL: [(&str, Self); 2] = [("for_pattern", Self::ForPattern), ("asm_block", Self::AsmBlock)];
}

fn emit(sources: &[Source<'_>]) -> Read<Vec<Data>> {
    let source = &grammar(sources);
    let tables = Tables { grammar: Grammar::read(source)?, source };
    let syntax = Syntax {
        name: LANGUAGE,
        extensions: vec![vun_span::EXTENSION],
        contexts: [
            vec![(Context::Main, main())],
            vec![(Context::Comments, lexical::comments())],
            lexical::strings(),
            vec![
                (Context::Literals, lexical::literals()),
                (Context::Keywords, tables.keywords()?),
                (Context::Names, tables.names()?),
                (Context::Punctuation, tables.punctuation()?),
            ],
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    Ok(vec![Data { name: FILE, format: Format::Yaml, text: syntax.text() }])
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

impl Enum<'_> {
    /// The spelling of the variant `name`.
    fn text(&self, name: &str) -> Read<&str> {
        let variant = self.variants.iter().find(|v| v.name == name);
        variant.and_then(|v| v.text.as_deref()).ok_or_else(|| Tables::missing(name))
    }
}

/// The grammar and the tables of its file.
struct Tables<'g, 's> {
    grammar: Grammar<'s>,
    source: &'g Source<'s>,
}

impl<'s> Tables<'_, 's> {
    fn missing(what: &str) -> Box<vun_diag::Diagnostic> {
        EDITOR.error(MISSING, what, Msg::new().text("the editor syntax reads it, but the grammar has none"))
    }

    fn kind(&self, name: &str) -> Read<&Enum<'s>> {
        self.grammar.enums.iter().find(|e| e.name == name).ok_or_else(|| Self::missing(name))
    }

    /// The spellings of the list `list` of the kind `kind`.
    fn texts(&self, kind: &str, list: &str) -> Read<Vec<&str>> {
        let kind = self.kind(kind)?;
        let (_, names) = kind.lists.iter().find(|(l, _)| *l == list).ok_or_else(|| Self::missing(list))?;
        names.iter().map(|&name| kind.text(name)).collect()
    }

    /// The spelling of the token `name`.
    fn token(&self, name: &str) -> Read<Pattern> {
        Ok(Pattern::literal(self.kind(TOKENS)?.text(name)?))
    }

    fn table(&self, name: &str) -> Read<&Literal<'s>> {
        let decl = self.source.decls.iter().find(|d| d.name == name);
        decl.map(|d| &d.value).ok_or_else(|| Self::missing(name))
    }

    /// Every `.name` a value holds, at any depth.
    fn mentioned(value: &Literal<'s>, into: &mut Vec<&'s str>) {
        match value {
            Literal::Name(name) => into.push(name),
            Literal::List(items) => items.iter().for_each(|v| Self::mentioned(v, into)),
            Literal::Record(fields) => fields.iter().for_each(|f| Self::mentioned(&f.value, into)),
            _ => {}
        }
    }
}
