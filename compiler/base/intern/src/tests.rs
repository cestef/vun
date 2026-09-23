use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::Symbol;

#[test]
fn interning_is_idempotent_across_threads() {
    let a = Symbol::intern("ring");
    let b = std::thread::spawn(|| Symbol::intern("ring")).join().unwrap();
    let c = Symbol::intern("buffer");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(c.as_str(), "buffer");
    assert!(c < a, "symbols order by text");
}

#[test]
fn hashes_depend_on_the_text_only() {
    let hash = |s: Symbol| {
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    // The hash is the same whether or not other names were interned first.
    let before = hash(Symbol::intern("gpio"));
    let _ = Symbol::intern("uart");
    assert_eq!(hash(Symbol::intern("gpio")), before);
}
