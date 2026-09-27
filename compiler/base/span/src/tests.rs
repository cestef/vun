use super::*;

#[test]
fn line_col_counts_from_one() {
    let file = SourceFile::new("a.vun".into(), "x := 1;\ny := 2;\n".into());
    assert_eq!(file.line_col(0), LineCol { line: 1, col: 1, chars: 1 });
    assert_eq!(file.line_col(8), LineCol { line: 2, col: 1, chars: 1 });
    assert_eq!(file.line_col(10), LineCol { line: 2, col: 3, chars: 3 });
}

#[test]
fn line_col_counts_characters_apart_from_bytes() {
    let file = SourceFile::new("a.vun".into(), "s := \"é\"; x".into());
    assert_eq!(file.line_col(11), LineCol { line: 1, col: 12, chars: 11 });
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
    let f = files.add("a.vun", "");
    assert_eq!(Span::new(f, 4, 6).cover(Span::new(f, 1, 5)), Span::new(f, 1, 6));
}

fn hal(path: &str) -> FileRef {
    let version = Version { major: 1, minor: 0, patch: 0 };
    let package = PackageId::Local { name: vun_intern::Symbol::intern("hal"), id: 7, version };
    FileRef { package, path: vun_intern::Symbol::intern(path) }
}

#[test]
fn file_ids_hash_by_key_not_by_order() {
    use std::hash::{BuildHasher, BuildHasherDefault};
    let hash =
        |id: FileId| BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(id);
    let first = SourceFiles::new();
    let a = first.add_as(hal("hal.vun"), "/home/a/hal/hal.vun", "");
    let second = SourceFiles::new();
    let _ = second.add("other.vun", "");
    let b = second.add_as(hal("hal.vun"), "/home/b/hal/hal.vun", "");
    assert_eq!(hash(a), hash(b));
    assert_eq!(second.file(b).path, "/home/b/hal/hal.vun");
}

#[test]
fn a_file_added_again_is_the_same_file() {
    let files = SourceFiles::new();
    let a = files.add_as(hal("a.vun"), "/x/a.vun", "x := 1;");
    assert_eq!(a, files.add_as(hal("a.vun"), "/x/a.vun", "x := 1;"));
    assert_ne!(a, files.add_as(hal("a.vun"), "/x/a.vun", "x := 2;"), "an edited file is another");
    let elsewhere = files.add_as(hal("a.vun"), "/y/a.vun", "x := 1;");
    assert_ne!(a, elsewhere, "each shows the path it was added with");
    assert_eq!(files.file(elsewhere).path, "/y/a.vun");
    assert_eq!(
        vun_hash::Hash::of_value(&a),
        vun_hash::Hash::of_value(&elsewhere),
        "named alike in every process"
    );
}

#[test]
fn a_fork_of_a_stored_release_is_named_apart_from_it() {
    let files = SourceFiles::new();
    let stored = |hash: &[u8]| FileRef {
        package: PackageId::Stored(vun_hash::Hash::of(hash)),
        path: vun_intern::Symbol::intern("hal.vun"),
    };
    let (release, fork) =
        (files.add_as(stored(b"release"), "/s/hal.vun", ""), files.add_as(stored(b"fork"), "/s/hal.vun", ""));
    assert_ne!(vun_hash::Hash::of_value(&release), vun_hash::Hash::of_value(&fork));
}

#[test]
fn versions_are_canonical_triples() {
    assert_eq!(Version::parse("1.20.3"), Some(Version { major: 1, minor: 20, patch: 3 }));
    for bad in ["1.2", "1.2.3.4", "01.2.3", "1.x.3", "", "1..3", "+1.2.3"] {
        assert_eq!(Version::parse(bad), None, "{bad}");
    }
    assert_eq!(Version::parse("0.4.2").unwrap().to_string(), "0.4.2");
}

#[test]
fn spans_read_back_only_within_their_text() {
    use vun_codec::{Context, Malformed, Writer};
    let file = SourceFiles::global().add("codec.vun", "é := 1;");
    let mut w = Writer::new();
    let bytes = [Span::new(file, 0, 2), Span::new(file, 0, 1), Span::new(file, 0, 99)].map(|s| w.encode(&s));
    let context = Context::new(w.into_pools());
    assert_eq!(context.read::<Span>(&bytes[0]), Ok(Span::new(file, 0, 2)));
    assert_eq!(context.read::<Span>(&bytes[1]), Err(Malformed::Invariant), "inside a character");
    assert_eq!(context.read::<Span>(&bytes[2]), Err(Malformed::Invariant), "past the text");
    let backwards = Writer::new().encode(&(3u32, 1u32));
    assert_eq!(
        Context::new(vun_codec::Pools::default()).read::<ByteRange>(&backwards),
        Err(Malformed::Invariant)
    );
}
