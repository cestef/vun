//! Queries: patterns over trees whose nodes they capture, written as the `.scm` files editors
//! read. A pattern's captures are of any type `C`, so that a caller builds them typed and maps
//! them to the names each editor reads ([`Pattern::map`]); they are written as `C` displays.

use std::fmt;

use vun_data::Value;
use vun_data::json::Json;

/// Patterns, each written on its own line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query<C> {
    pub patterns: Vec<Pattern<C>>,
}

/// A pattern and what it captures the node it matches as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern<C> {
    shape: Shape<C>,
    captures: Vec<C>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape<C> {
    /// A named node, of this kind or any, with children matching these, each in a field or any.
    Node(Option<String>, Vec<(Option<String>, Pattern<C>)>),
    /// An anonymous node: a token spelled so.
    Token(String),
    /// Any of these.
    Alternatives(Vec<Pattern<C>>),
    /// This pattern where the text captured as each capture matches its regular expression.
    Matching(Box<Pattern<C>>, Vec<(C, String)>),
}

impl<C> Pattern<C> {
    /// A node of the kind `kind`.
    #[must_use]
    pub fn node(kind: &str) -> Self {
        Self::of(Shape::Node(Some(kind.to_owned()), Vec::new()))
    }

    /// A named node of any kind.
    #[must_use]
    pub fn any() -> Self {
        Self::of(Shape::Node(None, Vec::new()))
    }

    /// The token spelled `text`.
    #[must_use]
    pub fn token(text: &str) -> Self {
        Self::of(Shape::Token(text.to_owned()))
    }

    /// Any of `patterns`; of one, that one.
    #[must_use]
    pub fn alternatives(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut patterns: Vec<Self> = patterns.into_iter().collect();
        if patterns.len() == 1 {
            return patterns.remove(0);
        }
        Self::of(Shape::Alternatives(patterns))
    }

    fn of(shape: Shape<C>) -> Self {
        Self { shape, captures: Vec::new() }
    }

    /// This node pattern with a child matching `child`, in the field `field` if one is named.
    ///
    /// # Panics
    /// Panics on a pattern that is not a node, which has no children.
    #[must_use]
    pub fn child(mut self, field: Option<&str>, child: Self) -> Self {
        let Shape::Node(_, children) = &mut self.shape else { panic!("only a node pattern has children") };
        children.push((field.map(str::to_owned), child));
        self
    }

    /// This pattern, capturing what it matches as `capture` too.
    #[must_use]
    pub fn capture(mut self, capture: C) -> Self {
        self.captures.push(capture);
        self
    }

    /// This pattern where the text captured as `capture` matches the regular expression `regex`.
    #[must_use]
    pub fn matching(self, capture: C, regex: &str) -> Self {
        Self::of(Shape::Matching(Box::new(self), vec![(capture, regex.to_owned())]))
    }

    /// This pattern with each capture mapped by `f`; `None` if `f` maps one to `None`.
    pub fn map<D>(self, f: &impl Fn(C) -> Option<D>) -> Option<Pattern<D>> {
        let shape = match self.shape {
            Shape::Node(kind, children) => Shape::Node(
                kind,
                children.into_iter().map(|(field, c)| Some((field, c.map(f)?))).collect::<Option<_>>()?,
            ),
            Shape::Token(text) => Shape::Token(text),
            Shape::Alternatives(ps) => {
                Shape::Alternatives(ps.into_iter().map(|p| p.map(f)).collect::<Option<_>>()?)
            }
            Shape::Matching(p, predicates) => Shape::Matching(
                Box::new(p.map(f)?),
                predicates.into_iter().map(|(c, r)| Some((f(c)?, r))).collect::<Option<_>>()?,
            ),
        };
        let captures = self.captures.into_iter().map(f).collect::<Option<_>>()?;
        Some(Pattern { shape, captures })
    }
}

impl<C: fmt::Display> fmt::Display for Pattern<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.shape {
            Shape::Node(kind, children) => {
                write!(f, "({}", kind.as_deref().unwrap_or("_"))?;
                for (field, child) in children {
                    match field {
                        Some(field) => write!(f, " {field}: {child}")?,
                        None => write!(f, " {child}")?,
                    }
                }
                f.write_str(")")?;
            }
            Shape::Token(text) => write!(f, "{}", Json(&Value::from(text.clone())))?,
            Shape::Alternatives(patterns) => {
                let patterns: Vec<String> = patterns.iter().map(ToString::to_string).collect();
                write!(f, "[{}]", patterns.join(" "))?;
            }
            Shape::Matching(pattern, predicates) => {
                write!(f, "({pattern}")?;
                for (capture, regex) in predicates {
                    write!(f, " (#match? @{capture} {})", Json(&Value::from(regex.clone())))?;
                }
                f.write_str(")")?;
            }
        }
        for capture in &self.captures {
            write!(f, " @{capture}")?;
        }
        Ok(())
    }
}

impl<C: fmt::Display> fmt::Display for Query<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for pattern in &self.patterns {
            writeln!(f, "{pattern}")?;
        }
        Ok(())
    }
}
