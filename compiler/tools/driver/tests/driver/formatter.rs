//! Formatters as components (decision 498): the formatter of Vun source loaded from its library
//! lays text out as the linked one does, and says when it does not parse.

use vun_driver::{Components, Provider, Registry};
use vun_formatter::{Formatter, Unlaid, WIDTH};

use crate::support::built_component;

const CRATE: &str = "vun-component-formatter-vun";

#[test]
fn a_formatter_loaded_from_a_library_lays_text_out_as_the_linked_one_does() {
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component(CRATE)));
    let loaded = registry.formatters.get("vun").expect("the library serves a formatter");
    assert!(matches!(loaded.from, Provider::Loaded(_)));

    let text = "f:=(a:u8,b:u8)->u8{a+b};\n";
    let linked = vun_fmt::Vun.layout(text, WIDTH).unwrap();
    assert_ne!(linked, text, "the text is not laid out already");
    assert_eq!(loaded.item.layout(text, WIDTH), Ok(linked));
    assert_eq!(loaded.item.layout("f := (", WIDTH), Err(Unlaid::Unparsed));
}
