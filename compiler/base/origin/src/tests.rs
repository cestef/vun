use super::*;
use overt_span::SourceFiles;

#[test]
fn chains_lead_back_to_source() {
    let f = SourceFiles::new().add("a.ov", "");
    let t = OriginTable::new();
    let written = t.source(Span::new(f, 10, 20));
    let desugared = t.derive(written, Span::new(f, 10, 20), Via::Desugar(DesugarRule(3)));
    let lowered = t.derive(desugared, Span::new(f, 12, 14), Via::Lower(PassId(1)));
    assert_eq!(t.chain(lowered).count(), 3);
    assert_eq!(t.source_span(lowered), Span::new(f, 10, 20));
}

#[test]
fn threads_record_origins_without_waiting() {
    let f = SourceFiles::new().add("a.ov", "");
    let t = OriginTable::new();
    std::thread::scope(|s| {
        for lo in 0..4 {
            let t = &t;
            s.spawn(move || {
                for i in 0..100 {
                    let _ = t.source(Span::new(f, lo, lo + i));
                }
            });
        }
    });
    assert_eq!(t.len(), 400);
}

#[test]
fn identities_survive_shifts_and_new_tables() {
    use overt_hash::Hash;
    use overt_intern::Symbol;
    let name = Symbol::intern("limit");
    // The same declaration, 40 bytes further down, read in another process.
    let [(first, a), (second, b)] =
        [(SourceFiles::new(), 10), (SourceFiles::new(), 50)].map(|(files, at)| {
            let f = files.add("pkg@1.0.0/pkg.ov", "");
            let t = OriginTable::new();
            let decl = t.declaration(Span::new(f, at, at + 10), None, name);
            let inner = t.inside(decl, Span::new(f, at + 2, at + 4));
            let desugared = t.derive(inner, Span::new(f, at + 2, at + 4), Via::Desugar(DesugarRule(1)));
            (t, [decl, inner, desugared])
        });
    assert_eq!(Hash::of_value(&a), Hash::of_value(&b));
    assert_eq!(first.source_span(a[2]).range.lo, 12);
    assert_eq!(second.source_span(b[2]).range.lo, 52);
    // Another name, or a node elsewhere in the declaration, is another origin.
    let f = SourceFiles::new().add("pkg@1.0.0/pkg.ov", "");
    let t = OriginTable::new();
    let other = t.declaration(Span::new(f, 10, 20), None, Symbol::intern("other"));
    assert_ne!(Hash::of_value(&other), Hash::of_value(&a[0]));
    let moved = t.inside(t.declaration(Span::new(f, 10, 20), None, name), Span::new(f, 13, 15));
    assert_ne!(Hash::of_value(&moved), Hash::of_value(&a[1]));
}
