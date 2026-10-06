use vun_component::Component;

use crate::{Formatter, Formatters, Remote, Unlaid};

/// Lays a text out as its length, or refuses what is empty.
struct Stub;

impl Formatter for Stub {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn layout(&self, text: &str, width: usize) -> Result<String, Unlaid> {
        if text.is_empty() {
            return Err(Unlaid::Unparsed);
        }
        assert!(text != "panic", "told to");
        Ok(format!("{}:{width}", text.len()))
    }
}

fn remote() -> Remote {
    Remote::of(Component::within(vun_component::descriptor!(Formatters, Stub)).unwrap()).unwrap()
}

#[test]
fn a_text_is_laid_out_through_the_boundary_with_the_width_asked() {
    let remote = remote();
    assert_eq!(remote.name(), "stub");
    assert_eq!(remote.layout("abc", 80), Ok("3:80".to_owned()));
    assert_eq!(remote.layout("", 80), Err(Unlaid::Unparsed));
}

#[test]
fn a_formatter_that_panics_is_a_fault_with_its_cause_and_is_called_again() {
    let remote = remote();
    assert!(matches!(remote.layout("panic", 80), Err(Unlaid::Faulted(_))));
    assert_eq!(remote.layout("ok", 80), Ok("2:80".to_owned()));
}
