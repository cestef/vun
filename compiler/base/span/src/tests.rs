use super::*;

#[test]
fn line_col_counts_from_one() {
    let file = SourceFile::new("a.ov".into(), "x := 1;\ny := 2;\n".into());
    assert_eq!(file.line_col(0), LineCol { line: 1, col: 1 });
    assert_eq!(file.line_col(8), LineCol { line: 2, col: 1 });
    assert_eq!(file.line_col(10), LineCol { line: 2, col: 3 });
}

#[test]
fn cover_joins_ranges() {
    let mut files = SourceFiles::new();
    let f = files.add("a.ov", "");
    assert_eq!(Span::new(f, 4, 6).cover(Span::new(f, 1, 5)), Span::new(f, 1, 6));
}
