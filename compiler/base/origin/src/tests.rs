use super::*;
use vun_span::SourceFiles;

#[test]
fn chains_lead_back_to_source() {
    let f = SourceFiles::new().add("a.vun", "");
    let t = OriginTable::new();
    let written = t.source(Span::new(f, 10, 20));
    let desugared = t.derive(written, Span::new(f, 10, 20), Via::Desugar(DesugarRule(3)));
    let lowered = t.derive(desugared, Span::new(f, 12, 14), Via::Lower(PassId(1)));
    assert_eq!(t.chain(lowered).count(), 3);
    assert_eq!(t.source_span(lowered), Span::new(f, 10, 20));
}

#[test]
fn threads_record_origins_without_waiting() {
    let f = SourceFiles::new().add("a.vun", "");
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
    use vun_hash::Hash;
    use vun_intern::Symbol;
    let name = Symbol::intern("limit");
    // The same declaration, 40 bytes further down, read in another process.
    let [(first, a), (second, b)] =
        [(SourceFiles::new(), 10), (SourceFiles::new(), 50)].map(|(files, at)| {
            let f = files.add("pkg@1.0.0/pkg.vun", "");
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
    let f = SourceFiles::new().add("pkg@1.0.0/pkg.vun", "");
    let t = OriginTable::new();
    let other = t.declaration(Span::new(f, 10, 20), None, Symbol::intern("other"));
    assert_ne!(Hash::of_value(&other), Hash::of_value(&a[0]));
    let moved = t.inside(t.declaration(Span::new(f, 10, 20), None, name), Span::new(f, 13, 15));
    assert_ne!(Hash::of_value(&moved), Hash::of_value(&a[1]));
}

#[test]
fn nodes_know_the_declaration_they_are_written_in() {
    let f = SourceFiles::new().add("a.vun", "");
    let t = OriginTable::new();
    let name = vun_intern::Symbol::intern("Point");
    let decl = t.declaration(Span::new(f, 0, 30), None, name);
    let literal = t.inside(decl, Span::new(f, 9, 30));
    let lowered = t.derive(literal, Span::new(f, 9, 30), Via::Lower(PassId(1)));
    assert_eq!([decl, literal, lowered].map(|o| t.declaration_name(o)), [Some(name); 3]);
    assert_eq!(t.declaration_name(t.source(Span::new(f, 0, 1))), None);
}

#[test]
fn origins_persist_by_content_and_keep_their_identity() {
    use vun_codec::{Context, Writer};
    use vun_span::SourceMap;

    let (files, origins) = (SourceFiles::global(), OriginTable::global());
    let f = files.add_as(vun_span::FileRef::loose("pkg/a.vun"), "a.vun", "f := 1;");
    let decl = origins.declaration(Span::new(f, 0, 7), None, Symbol::intern("f"));
    let lowered =
        origins.derive(origins.inside(decl, Span::new(f, 5, 6)), Span::new(f, 5, 6), Via::Lower(PassId(2)));
    let mut w = Writer::new();
    let bytes = w.encode(&lowered);

    // Read back, the chain is recorded anew with the same identities.
    let recorded = origins.len();
    let read: OriginId = Context::new(w.into_pools()).read(&bytes).unwrap();
    assert_eq!(origins.len(), recorded + 3, "recorded anew");
    assert_eq!(read, lowered, "the same identity");
    assert_eq!(vun_hash::Hash::of_value(&read), vun_hash::Hash::of_value(&lowered));
    let span = origins.source_span(read);
    assert_eq!(span.range.of(&files.file(span.file).text), "1");
    assert_eq!(origins.declaration_name(read), Some(Symbol::intern("f")));
}

/// A compilation reading `now` wherever `then` was read.
struct Reading<'t> {
    origins: &'t OriginTable,
    then: vun_span::FileId,
    now: vun_span::FileId,
}

impl Resolve for Reading<'_> {
    fn origins(&self) -> &OriginTable {
        self.origins
    }

    fn current(&self, file: vun_span::FileId) -> vun_span::FileId {
        if file == self.then { self.now } else { file }
    }
}

#[test]
fn an_origin_recorded_in_an_earlier_text_is_found_where_it_is_now() {
    use vun_intern::Symbol;
    let files = SourceFiles::new();
    let key = vun_span::FileRef::loose("a.vun");
    let (then, now) = (files.add_as(key.clone(), "a.vun", "then"), files.add_as(key, "a.vun", "now"));
    let t = OriginTable::new();
    let name = Symbol::intern("limit");
    let before = t.declaration(Span::new(then, 10, 40), None, name);
    let body = t.inside(before, Span::new(then, 20, 25));
    let lowered = t.derive(body, Span::new(then, 20, 25), Via::Lower(PassId(1)));
    // The text read again with 30 bytes more above the declaration.
    let after = t.declaration(Span::new(now, 40, 70), None, name);
    assert_eq!(before, after);
    let reading = Reading { origins: &t, then, now };
    assert_eq!(reading.span(before), Span::new(now, 40, 70));
    assert_eq!(reading.source_span(lowered), Span::new(now, 50, 55));
    // Read as recorded, as a compilation reading the earlier text does.
    assert_eq!(Resolve::source_span(&t, lowered), Span::new(then, 20, 25));
}

#[test]
fn a_file_shown_at_another_path_keeps_the_identities_of_its_origins() {
    use vun_intern::Symbol;
    use vun_span::FileRef;
    // One file, read from one fixture and from saved memos written by another at another path.
    let files = SourceFiles::new();
    let key = FileRef::loose("app.vun");
    let [here, saved] = ["/one/app.vun", "/two/app.vun"].map(|path| files.add_as(key.clone(), path, "text"));
    assert_ne!(here, saved);
    let t = OriginTable::new();
    let decl = t.declaration(Span::new(here, 10, 20), None, Symbol::intern("main"));
    assert_eq!(t.inside(decl, Span::new(here, 12, 14)), t.inside(decl, Span::new(saved, 12, 14)));
}
