//! URL schemes as components (decision 498): the https scheme loaded from its library is read
//! through like one linked in, and a failure keeps its causes.

use vun_component::Cause;
use vun_driver::{Components, Provider, Registry};
use vun_fetch::Failure;

use crate::common::built_component;

const CRATE: &str = "vun-component-scheme-https";

#[test]
fn a_scheme_loaded_from_a_library_reads_urls_and_reports_why_it_cannot() {
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component(CRATE)));
    assert!(matches!(registry.schemes.get("https").map(|s| &s.from), Some(Provider::Loaded(_))));
    let refused = "https://127.0.0.1:9/a.tar.gz".parse().unwrap();
    let Err(Failure::Download(error)) = registry.schemes().get(&refused) else {
        panic!("nothing listens there")
    };
    let cause = error.downcast_ref::<Cause>().expect("a failure of the library arrives as its causes");
    assert!(cause.messages().count() >= 2, "{cause:?}");
}
