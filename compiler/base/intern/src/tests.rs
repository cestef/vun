use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::{Interned, Pool, Symbol};

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

static PAIRS: Pool<(Symbol, u32)> = Pool::new();

#[test]
fn interned_values_are_shared_and_hash_by_content() {
    let a = PAIRS.intern((Symbol::intern("a"), 1));
    assert_eq!(a, PAIRS.intern((Symbol::intern("a"), 1)));
    assert_ne!(a, PAIRS.intern((Symbol::intern("a"), 2)));
    assert_eq!(a.1, 1);
    let hash = |v: Interned<(Symbol, u32)>| vun_hash::Hash::of_value(&v);
    assert_eq!(hash(a), hash(PAIRS.intern((Symbol::intern("a"), 1))));
}
