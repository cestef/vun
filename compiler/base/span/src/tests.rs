use super::*;

#[test]
fn line_col_counts_from_one() {
    let file = SourceFile::new("a.ov".into(), "x := 1;\ny := 2;\n".into());
    assert_eq!(file.line_col(0), LineCol { line: 1, col: 1 });
    assert_eq!(file.line_col(8), LineCol { line: 2, col: 1 });
    assert_eq!(file.line_col(10), LineCol { line: 2, col: 3 });
}

#[test]
fn lines_exclude_their_breaks() {
    let file = SourceFile::new("a".into(), "ab\r\n\ncd".into());
    let lines: Vec<_> = file.lines().collect();
    assert_eq!(
        lines,
        [("ab", ByteRange::new(0, 2)), ("", ByteRange::new(4, 4)), ("cd", ByteRange::new(5, 7))]
    );
}

#[test]
fn cover_joins_ranges() {
    let files = SourceFiles::new();
    let f = files.add("a.ov", "");
    assert_eq!(Span::new(f, 4, 6).cover(Span::new(f, 1, 5)), Span::new(f, 1, 6));
}

#[test]
fn file_ids_hash_by_key_not_by_order() {
    use std::hash::{BuildHasher, BuildHasherDefault};
    let hash =
        |id: FileId| BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(id);
    let first = SourceFiles::new();
    let a = first.add_as("hal@1.0.0/hal.ov", "/home/a/hal/hal.ov", "");
    let second = SourceFiles::new();
    let _ = second.add("other.ov", "");
    let b = second.add_as("hal@1.0.0/hal.ov", "/home/b/hal/hal.ov", "");
    assert_eq!(hash(a), hash(b));
    assert_eq!(second.file(b).path, "/home/b/hal/hal.ov");
}
