//! The shape of the item a derive is on: its name, generics, fields, variants and attributes, read
//! from its tokens. Types are never read, only skipped: the generated code names fields and lets
//! them infer.

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};

use crate::attr::Attrs;
use crate::depth::{Depth, Tree};
use crate::tokens::Tokens;

pub struct Item {
    pub name: Ident,
    pub attrs: Attrs,
    pub generics: Generics,
    pub body: Body,
}

#[derive(Default)]
pub struct Generics {
    pub params: Vec<Param>,
    /// The predicates of the item's `where` clause, each followed by a comma.
    pub predicates: Vec<TokenTree>,
}

pub struct Param {
    pub kind: ParamKind,
    /// The declaration without its default, as `T: Clone` or `const N: usize`.
    pub declaration: Vec<TokenTree>,
    /// The argument naming it, as `T` or `'a`.
    pub name: Vec<TokenTree>,
}

#[derive(PartialEq, Eq)]
pub enum ParamKind {
    Lifetime,
    Type,
    Const,
}

pub enum Body {
    Struct(Fields),
    Enum(Vec<Variant>),
}

pub struct Variant {
    pub name: Ident,
    pub attrs: Attrs,
    pub fields: Fields,
}

pub enum Fields {
    Named(Vec<Field>),
    Tuple(Vec<Field>),
    Unit,
}

/// A field: the name a pattern binds it to (its own, or `f0`, `f1`, ... in a tuple) and its
/// attributes.
pub struct Field {
    pub binding: Ident,
    pub attrs: Attrs,
}

/// Why the item cannot be derived for.
#[derive(Debug)]
pub struct Unsupported(pub &'static str);

impl Unsupported {
    /// `compile_error!("why")`.
    #[must_use]
    pub fn report(self) -> TokenStream {
        let message = proc_macro::Literal::string(self.0);
        Tokens::new()
            .path(&["core", "compile_error"])
            .op("!")
            .parens(Tokens::new().tree(message))
            .op(";")
            .into()
    }
}

/// The keyword introducing the item.
#[derive(PartialEq, Eq)]
enum Keyword {
    Struct,
    Enum,
    Union,
}

impl Item {
    /// # Errors
    /// The item is a union, or not an item.
    pub fn parse(input: TokenStream) -> Result<Self, Unsupported> {
        let trees: Vec<TokenTree> = input.into_iter().collect();
        // Attributes and visibility come before the keyword.
        let found = trees.iter().enumerate().find_map(|(at, tree)| Some((at, Keyword::of(tree)?)));
        let (at, keyword) = found.ok_or(Unsupported("expected a struct or an enum"))?;
        let attrs = Attrs::read(&trees[..at]);
        let mut trees = trees.into_iter().skip(at + 1).peekable();
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
            _ => return Err(Unsupported("unions cannot be derived for")),
        };
        Ok(Self { name, attrs, generics, body })
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
            let at = variant.iter().position(|tree| matches!(tree, TokenTree::Ident(_)));
            let Some(TokenTree::Ident(name)) = at.map(|at| &variant[at]) else {
                return Err(Unsupported("expected a variant"));
            };
            let at = at.expect("found a name");
            let fields = match variant.get(at + 1) {
                Some(TokenTree::Group(group)) => Fields::of(group),
                _ => Fields::Unit,
            };
            Ok(Variant { name: name.clone(), attrs: Attrs::read(&variant[..at]), fields })
        });
        variants.collect::<Result<_, _>>().map(Self::Enum)
    }
}

impl Fields {
    fn of(group: &Group) -> Self {
        let fields = Depth::split(group.stream().into_iter().collect(), true);
        match group.delimiter() {
            Delimiter::Brace => Self::Named(fields.iter().filter_map(|field| Field::named(field)).collect()),
            _ => {
                Self::Tuple(fields.iter().enumerate().map(|(i, field)| Field::positional(i, field)).collect())
            }
        }
    }

    /// Its fields, in order.
    #[must_use]
    pub fn all(&self) -> &[Field] {
        match self {
            Self::Named(fields) | Self::Tuple(fields) => fields,
            Self::Unit => &[],
        }
    }
}

impl Field {
    /// A named field: the identifier before its `:`, past attributes and visibility.
    fn named(field: &[TokenTree]) -> Option<Self> {
        let colon = field.iter().position(|tree| tree.is(':'))?;
        match &field[..colon] {
            [.., TokenTree::Ident(name)] => {
                Some(Self { binding: name.clone(), attrs: Attrs::read(&field[..colon]) })
            }
            _ => None,
        }
    }

    /// The field at position `i` of a tuple, bound as `f{i}`.
    fn positional(i: usize, field: &[TokenTree]) -> Self {
        Self { binding: Ident::new(&format!("f{i}"), Span::call_site()), attrs: Attrs::read(field) }
    }
}
