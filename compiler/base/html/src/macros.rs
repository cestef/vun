//! Writing documents the way they read. The macros expand to exactly the values of
//! [`crate::Node`] and [`crate::Attr`], so anything awkward to say in one can be written out and
//! mixed in with `[node]`.

/// A document as data, written as markup:
///
/// - `doctype`, the document type declaration;
/// - `"text"`, escaped text;
/// - `_`, a hole, filled when a template renders;
/// - `raw("<br>")`, markup written as it is;
/// - `[node]`, a node computed elsewhere, such as a custom element;
/// - `tag`, `tag [attrs]`, `tag { children }` or `tag [attrs] { children }`.
///
/// Attributes are `name`, `name = "value"` or `name = _`, separated by commas; tags and names are
/// the constants of [`crate::tag`] and [`crate::attr`].
///
/// ```
/// use vun_html::html;
///
/// const PAGE: vun_html::Doc = html! {
///     p [class = "greeting"] { "hello " strong { _ } "!" }
/// };
/// assert_eq!(PAGE.holes(), 1);
/// ```
#[macro_export]
macro_rules! html {
    ($($node:tt)*) => {
        $crate::Doc(&$crate::nodes!(@go [] $($node)*))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! nodes {
    (@go [$($done:expr,)*]) => { [$($done,)*] };
    (@go [$($done:expr,)*] doctype $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::Node::Doctype,] $($rest)*)
    };
    (@go [$($done:expr,)*] _ $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::Node::Hole,] $($rest)*)
    };
    (@go [$($done:expr,)*] raw($markup:expr) $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::Node::Raw($markup),] $($rest)*)
    };
    (@go [$($done:expr,)*] [$node:expr] $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $node,] $($rest)*)
    };
    (@go [$($done:expr,)*] $text:literal $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::Node::Text($text),] $($rest)*)
    };
    (@go [$($done:expr,)*] $tag:ident [$($attrs:tt)*] { $($children:tt)* } $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::nodes!(@element $tag [$($attrs)*] [$($children)*]),] $($rest)*)
    };
    (@go [$($done:expr,)*] $tag:ident [$($attrs:tt)*] $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::nodes!(@element $tag [$($attrs)*] []),] $($rest)*)
    };
    (@go [$($done:expr,)*] $tag:ident { $($children:tt)* } $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::nodes!(@element $tag [] [$($children)*]),] $($rest)*)
    };
    (@go [$($done:expr,)*] $tag:ident $($rest:tt)*) => {
        $crate::nodes!(@go [$($done,)* $crate::nodes!(@element $tag [] []),] $($rest)*)
    };
    (@element $tag:ident [$($attrs:tt)*] [$($children:tt)*]) => {
        $crate::Node::Element(
            $crate::tag::$tag,
            &$crate::attrs!(@go [] $($attrs)*),
            &$crate::nodes!(@go [] $($children)*),
        )
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! attrs {
    (@go [$($done:expr,)*]) => { [$($done,)*] };
    (@go [$($done:expr,)*] $name:ident = _ $(, $($rest:tt)*)?) => {
        $crate::attrs!(@go [$($done,)* $crate::Attr { name: $crate::attr::$name, value: $crate::Value::Hole },] $($($rest)*)?)
    };
    (@go [$($done:expr,)*] $name:ident = $value:expr $(, $($rest:tt)*)?) => {
        $crate::attrs!(@go [$($done,)* $crate::attr::$name.is($value),] $($($rest)*)?)
    };
    (@go [$($done:expr,)*] $name:ident $(, $($rest:tt)*)?) => {
        $crate::attrs!(@go [$($done,)* $crate::attr::$name.on(),] $($($rest)*)?)
    };
}

/// A static template, baked from a document written as [`html!`] writes one. Its type counts the
/// document's holes, and its buffer is sized from the document, so nothing is sized by hand.
///
/// ```
/// vun_html::template! {
///     static GREETING = { p { "hello " _ "!" } }
/// }
///
/// assert_eq!(GREETING.html(&("<world>",)), "<p>hello &lt;world&gt;!</p>");
/// ```
#[macro_export]
macro_rules! template {
    ($(#[$meta:meta])* $vis:vis static $name:ident = { $($doc:tt)* } $(;)?) => {
        $(#[$meta])*
        $vis static $name: $crate::Template<{ $crate::html!($($doc)*).holes() }> = {
            const DOC: $crate::Doc<'static> = $crate::html!($($doc)*);
            const BAKED: $crate::Baked<{ DOC.len() }, { DOC.holes() }> = $crate::Baked::of(&DOC);
            $crate::Template::new(&BAKED.bytes, BAKED.cuts)
        };
    };
}
