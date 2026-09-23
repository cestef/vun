use super::*;

#[test]
fn interning_is_idempotent() {
    let mut i = Interner::new();
    let a = i.intern("ring");
    let b = i.intern("ring");
    let c = i.intern("buffer");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(i.resolve(c), "buffer");
}
