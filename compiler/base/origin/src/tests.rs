use super::*;
use overt_span::SourceFiles;

#[test]
fn chains_lead_back_to_source() {
    let f = SourceFiles::new().add("a.ov", "");
    let mut t = OriginTable::new();
    let written = t.source(Span::new(f, 10, 20));
    let desugared = t.derive(written, Span::new(f, 10, 20), Via::Desugar(DesugarRule(3)));
    let lowered = t.derive(desugared, Span::new(f, 12, 14), Via::Lower(PassId(1)));
    assert_eq!(t.chain(lowered).count(), 3);
    assert_eq!(t.source_span(lowered), Span::new(f, 10, 20));
}
