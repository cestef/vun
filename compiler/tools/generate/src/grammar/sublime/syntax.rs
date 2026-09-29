//! A Sublime syntax as data (the format syntect, and so Typst, reads), written as JSON, which is
//! YAML too.

use vun_data::{Object, Value};

use crate::grammar::pattern::Pattern;

/// Ends every scope name, naming the language.
const LANGUAGE: &str = "vun";

/// The scopes a Vun syntax gives, named as editor themes select them; `editors.vun` names them
/// in `snake_case`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, vun_syntax::literal::FromLiteral)]
pub(crate) enum Scope {
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
pub(crate) struct Syntax {
    pub(super) name: &'static str,
    pub(super) extensions: Vec<&'static str>,
    pub(super) contexts: Vec<(Context, Vec<Rule>)>,
}

impl Syntax {
    /// The syntax as a JSON document, indented.
    pub(crate) fn text(&self) -> String {
        let contexts = self.contexts.iter().fold(Object::new(), |o, (context, rules)| {
            o.with(context.name(), rules.iter().map(Rule::value).collect::<Vec<_>>())
        });
        let syntax = Object::new()
            .with("name", self.name)
            .with("file_extensions", self.extensions.clone())
            .with("scope", Scope::Source.value())
            .with("contexts", contexts);
        format!("{:#}\n", vun_data::json::Json(&syntax))
    }

    /// The syntax as a `TextMate` grammar, which VS Code reads: each context a repository entry,
    /// and a match pushing a context a rule from it to the first of the context's pops.
    pub(crate) fn textmate(&self) -> String {
        let rules =
            |context: Context| self.contexts.iter().find(|(c, _)| *c == context).map(|(_, r)| r.as_slice());
        let patterns = |context| -> Vec<Value<'static>> {
            rules(context).unwrap_or_default().iter().filter_map(|r| Self::textmate_rule(r, &rules)).collect()
        };
        let repository =
            self.contexts.iter().filter(|(c, _)| *c != Context::Main).fold(Object::new(), |o, (c, _)| {
                o.with(c.name(), Object::new().with("patterns", patterns(*c)))
            });
        let grammar = Object::new()
            .with("name", self.name)
            .with("scopeName", Scope::Source.value())
            .with("fileTypes", self.extensions.clone())
            .with("patterns", patterns(Context::Main))
            .with("repository", repository);
        format!("{:#}\n", vun_data::json::Json(&grammar))
    }

    fn textmate_rule<'r>(
        rule: &Rule,
        rules: &impl Fn(Context) -> Option<&'r [Rule]>,
    ) -> Option<Value<'static>> {
        let named = |scope: &Scope| Object::new().with("0", Object::new().with("name", scope.value()));
        Some(match rule {
            Rule::Include(context) => Object::new().with("include", format!("#{}", context.name())).into(),
            Rule::MetaScope(_) => return None,
            Rule::Match { pattern, scope, then: Then::Stay | Then::Pop } => {
                Object::new().with("match", pattern.to_string()).with("name", scope.value()).into()
            }
            Rule::Match { pattern, scope, then: Then::Push(context) } => {
                let inside = rules(*context).unwrap_or_default();
                let pops: Vec<Pattern> = inside
                    .iter()
                    .filter_map(|r| match r {
                        Rule::Match { pattern, then: Then::Pop, .. } => Some(pattern.clone()),
                        _ => None,
                    })
                    .collect();
                let meta = inside.iter().find_map(|r| match r {
                    Rule::MetaScope(scope) => Some(scope.value()),
                    _ => None,
                });
                let nested: Vec<Value<'static>> = inside
                    .iter()
                    .filter(|r| !matches!(r, Rule::Match { then: Then::Pop, .. }))
                    .filter_map(|r| Self::textmate_rule(r, rules))
                    .collect();
                let mut object = Object::new()
                    .with("begin", pattern.to_string())
                    .with("beginCaptures", named(scope))
                    .with("end", Pattern::any(pops).to_string())
                    .with("patterns", nested);
                if let Some(meta) = meta {
                    object.push("name", meta);
                }
                object.into()
            }
        })
    }
}
