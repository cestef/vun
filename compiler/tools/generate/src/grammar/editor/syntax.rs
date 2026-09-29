//! A Sublime syntax as data (the format syntect, and so Typst, reads), written as JSON, which is
//! YAML too.

use vun_json::{Object, Value};

use super::pattern::Pattern;

/// Ends every scope name, naming the language.
const LANGUAGE: &str = "vun";

/// The scopes a Vun syntax gives, named as editor themes select them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Scope {
    Source,
    Comment,
    DocComment,
    String,
    RawString,
    RawLine,
    StringBegin,
    StringEnd,
    Char,
    Escape,
    InvalidEscape,
    Integer,
    Number,
    Keyword,
    Contextual,
    Borrow,
    Flow,
    Declaration,
    Assignment,
    Operator,
    Stage,
    Quote,
    Annotation,
    Section,
    Separator,
    Punctuation,
    Hole,
    Escaped,
    Label,
    Type,
    Definition,
    Call,
    Member,
}

impl Scope {
    fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Comment => "comment.line.double-slash",
            Self::DocComment => "comment.line.documentation",
            Self::String => "string.quoted.double",
            Self::RawString => "string.quoted.raw",
            Self::RawLine => "string.unquoted.raw",
            Self::StringBegin => "punctuation.definition.string.begin",
            Self::StringEnd => "punctuation.definition.string.end",
            Self::Char => "constant.character",
            Self::Escape => "constant.character.escape",
            Self::InvalidEscape => "invalid.illegal.escape",
            Self::Integer => "constant.numeric.integer",
            Self::Number => "constant.numeric",
            Self::Keyword => "keyword.control",
            Self::Contextual => "keyword.other",
            Self::Borrow => "keyword.other.borrow",
            Self::Flow => "keyword.other.flow",
            Self::Declaration => "keyword.operator.declaration",
            Self::Assignment => "keyword.operator.assignment",
            Self::Operator => "keyword.operator",
            Self::Stage => "keyword.operator.stage",
            Self::Quote => "keyword.operator.quote",
            Self::Annotation => "punctuation.definition.annotation.begin",
            Self::Section => "punctuation.section",
            Self::Separator => "punctuation.separator",
            Self::Punctuation => "punctuation.other",
            Self::Hole => "constant.language.hole",
            Self::Escaped => "variable.other.escaped",
            Self::Label => "entity.name.label",
            Self::Type => "entity.name.type",
            Self::Definition => "entity.name.function",
            Self::Call => "variable.function",
            Self::Member => "variable.other.member",
        }
    }

    fn value(self) -> Value<'static> {
        Value::from([self.name(), LANGUAGE].join("."))
    }
}

/// The contexts of the syntax, `Main` the one a file starts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    Main,
    Comments,
    Strings,
    String,
    RawString,
    Literals,
    Keywords,
    Names,
    Punctuation,
}

impl Context {
    fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Comments => "comments",
            Self::Strings => "strings",
            Self::String => "string",
            Self::RawString => "raw_string",
            Self::Literals => "literals",
            Self::Keywords => "keywords",
            Self::Names => "names",
            Self::Punctuation => "punctuation",
        }
    }
}

/// A rule of a context, tried in order where no earlier match starts sooner.
pub(super) enum Rule {
    Include(Context),
    /// The scope of everything inside the context.
    MetaScope(Scope),
    Match {
        pattern: Pattern,
        scope: Scope,
        then: Then,
    },
}

/// What a match does to the stack of contexts.
pub(super) enum Then {
    Stay,
    Push(Context),
    Pop,
}

impl Rule {
    /// `pattern` given `scope`.
    pub(super) fn of(pattern: Pattern, scope: Scope) -> Self {
        Self::Match { pattern, scope, then: Then::Stay }
    }

    /// This rule, then `then`.
    #[must_use]
    pub(super) fn then(self, next: Then) -> Self {
        match self {
            Self::Match { pattern, scope, .. } => Self::Match { pattern, scope, then: next },
            other => other,
        }
    }

    fn value(&self) -> Value<'static> {
        match self {
            Self::Include(context) => Object::new().with("include", context.name()).into(),
            Self::MetaScope(scope) => Object::new().with("meta_scope", scope.value()).into(),
            Self::Match { pattern, scope, then } => {
                let mut rule = Object::new().with("match", pattern.to_string()).with("scope", scope.value());
                match then {
                    Then::Stay => {}
                    Then::Push(context) => rule.push("push", context.name()),
                    Then::Pop => rule.push("pop", true),
                }
                rule.into()
            }
        }
    }
}

/// A whole syntax.
pub(super) struct Syntax {
    pub(super) name: &'static str,
    pub(super) extensions: Vec<&'static str>,
    pub(super) contexts: Vec<(Context, Vec<Rule>)>,
}

impl Syntax {
    /// The syntax as a JSON document, indented.
    pub(super) fn text(&self) -> String {
        let contexts = self.contexts.iter().fold(Object::new(), |o, (context, rules)| {
            o.with(context.name(), rules.iter().map(Rule::value).collect::<Vec<_>>())
        });
        let syntax = Object::new()
            .with("name", self.name)
            .with("file_extensions", self.extensions.clone())
            .with("scope", Scope::Source.value())
            .with("contexts", contexts);
        format!("{syntax:#}\n")
    }
}
