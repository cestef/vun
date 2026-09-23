use overt_diag::{DiagCtxt, DiagCx, DiagId, errors};
use overt_harness::{discover, repo_root};
use overt_origin::OriginTable;
use overt_span::{EXTENSION, FileId, SourceFiles};
use overt_syntax::literal::{Decl, declarations};
use overt_syntax::{lexer, parser};

use super::{DepKind, Manifest, Source, Version, declared, read};
use crate::layout::MANIFEST;

fn parsed<R>(text: &str, f: impl FnOnce(&[Decl<'_>], FileId, &mut DiagCx<'_>) -> R) -> (R, Vec<DiagId>) {
    let file = SourceFiles::new().add(MANIFEST, text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = lexer::lex(file, text, &mut cx);
    let cst = parser::parse(&tokens, text, &mut cx);
    let result = f(&declarations(&cst, &tokens, text), file, &mut cx);
    (result, diag.sorted().into_iter().map(|d| d.id.clone()).collect())
}

fn declares(text: &str) -> bool {
    parsed(text, |decls, _, _| declared(decls)).0
}

fn manifest(text: &str) -> (Option<Manifest>, Vec<DiagId>) {
    parsed(text, read)
}

#[test]
fn every_manifest_reads() {
    let root = repo_root();
    let mut read = 0;
    for dir in ["lib", "corpus"] {
        for path in discover(&root.join(dir), EXTENSION).unwrap() {
            let text = std::fs::read_to_string(&path).unwrap();
            if path.file_name().is_some_and(|n| n == MANIFEST) && declares(&text) {
                let (read_once, ids) = manifest(&text);
                assert!(read_once.is_some() && ids.is_empty(), "{}: {ids:?}", path.display());
                // What the writer writes reads back as the same manifest.
                let written = read_once.unwrap().source().unwrap();
                let (again, ids) = manifest(&written);
                assert_eq!(
                    (again.unwrap().source().unwrap(), ids),
                    (written, Vec::new()),
                    "{}",
                    path.display()
                );
                read += 1;
            }
        }
    }
    assert_eq!(read, 9, "manifests found");
}

#[test]
fn reads_every_field() {
    let text = r#"package := .{
        .name = "blinky", .id = 0x6a80, .version = "0.3.1", .edition = 2026, .toolchain = "0.4.2",
        .include = .{ "build.ov", "blinky" }, .prelude = .{ "std.prelude", "support" },
        .deps = .{
            .hal = .{ .version = "1.3.0", .hash = "blake3:k3f2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", .urls = .{ "https://x" } },
            .cmsis = .{ .version = "5.9.0", .hash = "blake3:m7qdaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", .kind = .foreign },
            .util = .{ .path = "../util" },
        },
    };"#;
    let (Some(m), ids) = manifest(text) else { panic!("unread") };
    assert!(ids.is_empty(), "{ids:?}");
    assert_eq!((m.name.as_str(), m.id, m.edition), ("blinky", 0x6a80, 2026));
    assert_eq!(m.toolchain, Version::parse("0.4.2"));
    assert_eq!(m.include, ["build.ov", "blinky"]);
    assert_eq!(m.prelude, ["std.prelude", "support"]);
    let deps: Vec<_> = m.deps.iter().map(|d| (d.name.as_str(), d.kind)).collect();
    assert_eq!(deps, [("hal", DepKind::Overt), ("cmsis", DepKind::Foreign), ("util", DepKind::Overt)]);
    assert_eq!(m.deps[2].source, Source::Path("../util".into()));
}

#[test]
fn reports_where_the_manifest_is_wrong() {
    let cases: &[(&str, &[DiagId])] = &[
        ("build := 1;", &[errors::MANIFEST_MISSING_PACKAGE]),
        ("package := 1;", &[errors::MANIFEST_INVALID_FIELD]),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{},
                .deps = .{ .x = .{ .version = "1.0.0", .hash = "blake3-m7qd" } } };"#,
            &[errors::MANIFEST_INVALID_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0", .edition = 1, .include = .{} };"#,
            &[errors::MANIFEST_INVALID_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = -1, .version = "1.0.0", .edition = 1, .include = .{}, .dpes = .{} };"#,
            &[errors::MANIFEST_INVALID_FIELD, errors::MANIFEST_UNKNOWN_FIELD],
        ),
        (
            r#"package := .{ .nme = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{} };"#,
            &[errors::MANIFEST_MISSING_FIELD, errors::MANIFEST_UNKNOWN_FIELD],
        ),
        (
            r#"package := .{ .name = "a", .id = 1, .version = "1.0.0", .edition = 1, .include = .{},
                .deps = .{ .x = .{ .path = "p", .version = "1.0.0" }, .y = .{ .version = "1.0.0" } } };"#,
            &[errors::MANIFEST_INVALID_FIELD, errors::MANIFEST_MISSING_FIELD],
        ),
    ];
    for &(text, expected) in cases {
        assert_eq!(manifest(text).1, expected, "{text}");
    }
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
fn declared_deps_read_back() {
    let hash = overt_hash::Hash::of(b"hal");
    let archive = Source::Archive {
        version: Version::parse("1.2.0").unwrap(),
        hash,
        urls: vec!["https://example.org/hal-1.2.0.ovpkg".into()],
    };
    let wanted =
        [("util", Source::Path("../util".into()), DepKind::Overt), ("hal", archive, DepKind::Foreign)];
    let mut text =
        "package := .{\n    .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2026,\n    \
                    .include = .{ \"build.ov\" },\n};\n"
            .to_owned();
    for (name, source, kind) in &wanted {
        let edit = parsed(&text, |decls, file, _| {
            let dep = super::Dep {
                name: (*name).into(),
                source: source.clone(),
                kind: *kind,
                span: overt_span::Span::new(file, 0, 0),
            };
            dep.declare(decls, &text).unwrap()
        })
        .0;
        text = overt_diag::Edit::apply(&[edit], &text);
    }
    let (manifest, ids) = manifest(&text);
    assert!(ids.is_empty(), "{ids:?}\n{text}");
    let deps: Vec<_> = manifest.unwrap().deps.into_iter().map(|d| (d.name, d.source, d.kind)).collect();
    let wanted: Vec<_> = wanted.into_iter().map(|(n, s, k)| (n.to_owned(), s, k)).collect();
    assert_eq!(deps, wanted, "{text}");
}
