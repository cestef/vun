//! Any text either is a URI reference or fails with a reason; a reference verifies, reads back as
//! written, normalizes to a fixed point, and resolves to a URI that verifies.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_uri::{Reference, Uri};

/// The base of RFC 3986's examples.
const BASE: &str = "http://a/b/c/d;p?q";

fuzz_target!(|text: &str| {
    let Ok(reference) = text.parse::<Reference>() else { return };
    reference.verify().unwrap();
    assert_eq!(reference.as_str(), text);
    let normalized = reference.normalized();
    normalized.verify().unwrap();
    assert_eq!(normalized.normalized(), normalized);
    let base: Uri = BASE.parse().unwrap();
    let resolved = base.resolve(&reference);
    resolved.verify().unwrap();
    resolved.normalized().verify().unwrap();
});
