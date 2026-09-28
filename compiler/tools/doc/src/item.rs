//! What a page documents: declarations with their doc comments, attributes and signatures.

/// A public declaration of a namespace, or a member of one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub kind: Kind,
    /// The doc comment's lines, Markdown.
    pub doc: Vec<String>,
    /// The attributes, as written: `#[link.export("main")]`.
    pub attributes: Vec<String>,
    /// The declaration without its body, on one line: `f := (x: u8) -> u8`.
    pub signature: String,
    /// Fields, variants and declarations of a type, in source order.
    pub members: Vec<Item>,
    /// The line it is declared on, from 1.
    pub line: u32,
}

/// What a declaration is, as its value's shape says; a caller that types the declarations tells
/// the types among the rest (`Namespace::retype`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A function literal, or a declaration typed as a function.
    Function,
    /// A `struct` literal, or what makes one: a type constructor, an interface.
    Struct,
    /// An `enum` literal, or what makes one.
    Enum,
    /// A field of a `struct`.
    Field,
    /// A variant of an `enum`.
    Variant,
    /// A type whose value's shape does not show it: `Word := u32`, `Pair := Make(u8)`.
    Type,
    /// Any other value.
    Value,
}

impl Kind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Field => "field",
            Self::Variant => "variant",
            Self::Type => "type",
            Self::Value => "value",
        }
    }
}

impl Item {
    /// The first paragraph of the doc comment, joined on one line.
    #[must_use]
    pub fn summary(doc: &[String]) -> String {
        let lines = doc.iter().take_while(|l| !l.trim().is_empty());
        lines.map(|l| l.trim()).collect::<Vec<_>>().join(" ")
    }
}

impl Item {
    /// The anchor of a declaration named `name` inside the declarations `outer`, outermost
    /// first, none for a namespace's own: `Point.x`.
    #[must_use]
    pub fn anchor(outer: &[&str], name: &str) -> String {
        let mut path = outer.to_vec();
        path.push(name);
        path.join(vun_grammar::PATH_SEPARATOR)
    }
}
