//! Typed HTML, after `press`: a page is a value, not a string. An element carries a [`Tag`],
//! attributes and a child that renders itself, so an unclosed element is a type error rather
//! than something a browser discovers, and text escapes itself.
//!
//! Two front ends share one vocabulary:
//!
//! - **Run time.** [`Element`], [`Text`], [`Raw`] and the [`Render`] implementations on
//!   strings, numbers, options, tuples (siblings), slices and iterators ([`Each`]) compose a
//!   document from values known only then. The child is a type parameter, so rendering is a
//!   tree of calls the compiler flattens.
//! - **Compile time.** [`html!`] writes a [`Doc`] as data; [`template!`] has the Rust compiler
//!   walk it and bake it into one string with the places of its holes (`_`). Rendering copies
//!   the text between holes and renders each fill: work follows the holes, not the page. The
//!   template's type counts its holes, and its buffer is sized from the document.
//!
//! ```
//! use vun_html::{Render, attr, tag};
//!
//! let items = ["one", "<two>"];
//! let list = tag::ul.attrs([attr::class.is("items")]).with(vun_html::Each(
//!     items.iter().map(|item| tag::li.with(*item)),
//! ));
//! assert_eq!(list.html(), r#"<ul class="items"><li>one</li><li>&lt;two&gt;</li></ul>"#);
//! ```
//!
//! Text escapes what would end an element or an attribute; this is not a sanitiser, and [`Raw`]
//! escapes nothing by design.

pub mod attr;
mod element;
mod escape;
mod macros;
mod node;
mod render;
pub mod tag;
mod template;
#[cfg(test)]
mod tests;

pub use attr::{Attr, Name, Value};
pub use element::{Doctype, Element, Raw, Text};
pub use escape::Escape;
pub use node::{Cut, Doc, Node};
pub use render::{Each, Render};
pub use tag::{Kind, Tag};
#[doc(hidden)]
pub use template::Baked;
pub use template::{Fill, Template};
