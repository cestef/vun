use overt_diag::{DiagCtxt, DiagCx, DiagId, errors};
use overt_harness::{discover, repo_root};
use overt_origin::OriginTable;
use overt_span::SourceFiles;
use overt_syntax::literal::declarations;
use overt_syntax::{lexer, parser};

use super::{DepKind, Manifest, Source, Version, read};

fn manifest(text: &str) -> (Option<Manifest>, Vec<DiagId>) {
    let file = SourceFiles::new().add("build.ov", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
    let tokens = lexer::lex(file, text, &mut cx);
    let cst = parser::parse(&tokens, text, &mut cx);
    let manifest = read(&declarations(&cst, &tokens, text), file, &mut cx);
    (manifest, diag.sorted().into_iter().map(|d| d.id.clone()).collect())
}

#[test]
fn every_build_file_reads() {
    let root = repo_root();
    let mut read = 0;
    for dir in ["lib", "corpus"] {
        for path in discover(&root.join(dir), "ov").unwrap() {
            // A manifest sits next to its package namespace `<dir>.ov`; `std/std/build.ov` is a module.
            let dir = path.parent().unwrap();
            let namespace = dir.join(dir.file_name().unwrap()).with_extension("ov");
            if path.file_name().is_some_and(|n| n == "build.ov") && namespace.exists() {
                let text = std::fs::read_to_string(&path).unwrap();
                let (manifest, ids) = manifest(&text);
                assert!(manifest.is_some() && ids.is_empty(), "{}: {ids:?}", path.display());
                read += 1;
            }
        }
    }
    assert_eq!(read, 7, "manifests found");
}

#[test]
fn reads_every_field() {
    let text = r#"package := .{
        .name = "blinky", .id = 0x6a80, .version = "0.3.1", .edition = 2027, .toolchain = "0.4.2",
        .include = .{ "build.ov", "blinky" },
        .deps = .{
            .hal = .{ .version = "1.3.0", .hash = "sha256-9f2c", .urls = .{ "https://x" } },
            .cmsis = .{ .version = "5.9.0", .hash = "sha256-11ab", .kind = .foreign },
            .util = .{ .path = "../util" },
        },
    };"#;
    let (Some(m), ids) = manifest(text) else { panic!("unread") };
    assert!(ids.is_empty(), "{ids:?}");
    assert_eq!((m.name.as_str(), m.id, m.edition), ("blinky", 0x6a80, 2027));
    assert_eq!(m.toolchain, Version::parse("0.4.2"));
    assert_eq!(m.include, ["build.ov", "blinky"]);
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
