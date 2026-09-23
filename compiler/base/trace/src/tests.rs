use tracing::Level;

use crate::Filter;

#[test]
fn the_most_specific_directive_decides() {
    let filter = Filter::parse("info, overt_sema=trace, overt_sema::int=error, bogus=loud");
    assert!(filter.enabled("overt_sema::check", Level::TRACE));
    assert!(!filter.enabled("overt_sema::int", Level::WARN));
    assert!(filter.enabled("overt_syntax", Level::INFO));
    assert!(!filter.enabled("overt_syntax", Level::DEBUG));
    assert!(Filter::default().enabled("anything", Level::WARN));
    assert!(!Filter::default().enabled("anything", Level::INFO));
    assert!(
        Filter::parse("debug,warn").enabled("x", Level::WARN)
            && !Filter::parse("debug,warn").enabled("x", Level::INFO),
        "later wins"
    );
}
