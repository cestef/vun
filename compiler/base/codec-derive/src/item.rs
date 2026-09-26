//! The shape of the item a derive is on: its name, generics and fields, read from its tokens.
//! Types are never read, only skipped: the generated code names fields and lets them infer.

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, TokenStream, TokenTree};

pub(crate) struct Item {
    pub(crate) name: Ident,
    pub(crate) generics: Generics,
    pub(crate) body: Body,
}

#[derive(Default)]
pub(crate) struct Generics {
    pub(crate) params: Vec<Param>,
    /// The predicates of the item's `where` clause, each followed by a comma.
    pub(crate) predicates: Vec<TokenTree>,
}

pub(crate) struct Param {
    pub(crate) kind: ParamKind,
    /// The declaration without its default, as `T: Clone` or `const N: usize`.
    pub(crate) declaration: Vec<TokenTree>,
    /// The argument naming it, as `T` or `'a`.
    pub(crate) name: Vec<TokenTree>,
}

#[derive(PartialEq, Eq)]
pub(crate) enum ParamKind {
    Lifetime,
    Type,
    Const,
}

pub(crate) enum Body {
    Struct(Fields),
    Enum(Vec<(Ident, Fields)>),
}

pub(crate) enum Fields {
    Named(Vec<Ident>),
    Tuple(usize),
    Unit,
}

/// Why the item cannot be derived for.
pub(crate) struct Unsupported(pub &'static str);

/// The keyword introducing the item.
#[derive(PartialEq, Eq)]
enum Keyword {
    Struct,
    Enum,
    Union,
}

impl Item {
    pub(crate) fn parse(input: TokenStream) -> Result<Self, Unsupported> {
        let mut trees = input.into_iter().peekable();
        // Attributes and visibility come before the keyword.
        let keyword = trees
            .by_ref()
            .find_map(|tree| Keyword::of(&tree))
            .ok_or(Unsupported("expected a struct or an enum"))?;
        let Some(TokenTree::Ident(name)) = trees.next() else { return Err(Unsupported("expected a name")) };
        let mut generics = Generics::default();
        if trees.next_if(|tree| tree.is('<')).is_some() {
            generics.params =
                Depth::split(Depth::angled(&mut trees), true).into_iter().map(Param::parse).collect();
        }
        let rest: Vec<TokenTree> = trees.collect();
        let fields_at = rest.iter().position(|tree| {
            matches!(tree, TokenTree::Group(g) if matches!(g.delimiter(), Delimiter::Brace | Delimiter::Parenthesis))
        });
        let (fields, clause) = match fields_at {
            Some(at) => {
                let TokenTree::Group(group) = &rest[at] else { unreachable!("found a group") };
                // A tuple struct's `where` follows its fields; any other item's precedes them.
                let clause =
                    if group.delimiter() == Delimiter::Parenthesis { &rest[at + 1..] } else { &rest[..at] };
                (Some(group), clause)
            }
            None => (None, &rest[..]),
        };
        generics.predicates = Generics::predicates(clause);
        let body = match (keyword, fields) {
            (Keyword::Struct, fields) => Body::Struct(fields.map_or(Fields::Unit, Fields::of)),
            (Keyword::Enum, Some(variants)) => Body::variants(variants)?,
            _ => return Err(Unsupported("unions are not encoded")),
        };
        Ok(Self { name, generics, body })
    }
}

impl Keyword {
    fn of(tree: &TokenTree) -> Option<Self> {
        let TokenTree::Ident(ident) = tree else { return None };
        match ident.to_string().as_str() {
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "union" => Some(Self::Union),
            _ => None,
        }
    }
}

impl Generics {
    /// The predicates of a `where` clause, followed by a comma.
    fn predicates(clause: &[TokenTree]) -> Vec<TokenTree> {
        let mut trees: Vec<TokenTree> =
            clause.iter().skip_while(|tree| !tree.is_word("where")).skip(1).cloned().collect();
        if trees.last().is_some_and(|tree| tree.is(';')) {
            trees.pop();
        }
        if trees.last().is_some_and(|tree| !tree.is(',')) {
            trees.push(Punct::new(',', Spacing::Alone).into());
        }
        trees
    }
}

impl Param {
    fn parse(mut declaration: Vec<TokenTree>) -> Self {
        if let Some(at) = declaration.iter().position(|tree| tree.is('=')) {
            declaration.truncate(at);
        }
        let (kind, name) = match declaration.as_slice() {
            [lifetime @ TokenTree::Punct(_), name, ..] => {
                (ParamKind::Lifetime, vec![lifetime.clone(), name.clone()])
            }
            [keyword, name, ..] if keyword.is_word("const") => (ParamKind::Const, vec![name.clone()]),
            [name, ..] => (ParamKind::Type, vec![name.clone()]),
            [] => (ParamKind::Type, Vec::new()),
        };
        Self { kind, declaration, name }
    }
}

impl Body {
    fn variants(body: &Group) -> Result<Self, Unsupported> {
        let variants = Depth::split(body.stream().into_iter().collect(), false);
        let variants = variants.iter().map(|variant| {
            // Past its attributes, a variant is its name, then its fields if any.
            let mut trees = variant.iter().skip_while(|tree| !matches!(tree, TokenTree::Ident(_)));
            let Some(TokenTree::Ident(name)) = trees.next() else {
                return Err(Unsupported("expected a variant"));
            };
            let fields = match trees.next() {
                Some(TokenTree::Group(group)) => Fields::of(group),
                _ => Fields::Unit,
            };
            Ok((name.clone(), fields))
        });
        variants.collect::<Result<_, _>>().map(Self::Enum)
    }
}

impl Fields {
    fn of(group: &Group) -> Self {
        let fields = Depth::split(group.stream().into_iter().collect(), true);
        match group.delimiter() {
            Delimiter::Brace => Self::Named(fields.iter().filter_map(|field| Self::name(field)).collect()),
            _ => Self::Tuple(fields.len()),
        }
    }

    /// The name of a named field: the identifier before its `:`, past attributes and visibility.
    fn name(field: &[TokenTree]) -> Option<Ident> {
        let colon = field.iter().position(|tree| tree.is(':'))?;
        match &field[..colon] {
            [.., TokenTree::Ident(name)] => Some(name.clone()),
            _ => None,
        }
    }
}

/// The nesting of angle brackets, which are punctuation rather than groups; the `>` of `->` does
/// not close one.
#[derive(Default)]
struct Depth {
    level: usize,
    arrow: bool,
}

impl Depth {
    /// The trees up to the `>` closing an angle bracket already opened.
    fn angled(trees: &mut impl Iterator<Item = TokenTree>) -> Vec<TokenTree> {
        let mut depth = Self::default();
        trees.take_while(|tree| depth.step(tree)).collect()
    }

    /// `trees` split at their commas, those inside angle brackets aside when `angles`; empty
    /// segments, as after a trailing comma, are dropped.
    fn split(trees: Vec<TokenTree>, angles: bool) -> Vec<Vec<TokenTree>> {
        let mut depth = Self::default();
        let mut segments = vec![Vec::new()];
        for tree in trees {
            if angles {
                depth.step(&tree);
            }
            if depth.level == 0 && tree.is(',') {
                segments.push(Vec::new());
            } else {
                segments.last_mut().expect("one segment at least").push(tree);
            }
        }
        segments.retain(|segment| !segment.is_empty());
        segments
    }

    /// Steps over `tree`, returning whether it is inside the brackets it started in.
    fn step(&mut self, tree: &TokenTree) -> bool {
        let arrow = std::mem::take(&mut self.arrow);
        let TokenTree::Punct(punct) = tree else { return true };
        match punct.as_char() {
            '<' => self.level += 1,
            '>' if !arrow => match self.level.checked_sub(1) {
                Some(level) => self.level = level,
                None => return false,
            },
            '-' => self.arrow = punct.spacing() == Spacing::Joint,
            _ => {}
        }
        true
    }
}

/// Questions about one token.
trait Tree {
    fn is(&self, c: char) -> bool;
    fn is_word(&self, word: &str) -> bool;
}

impl Tree for TokenTree {
    fn is(&self, c: char) -> bool {
        matches!(self, Self::Punct(p) if p.as_char() == c)
    }

    fn is_word(&self, word: &str) -> bool {
        matches!(self, Self::Ident(i) if i.to_string() == word)
    }
}
