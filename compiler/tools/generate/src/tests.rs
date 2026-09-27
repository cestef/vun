use vun_harness::repo_root;

use crate::generate;

#[test]
fn generated_sources_are_current() {
    let root = repo_root();
    for generator in generate::ALL {
        let generated = generator.generate(&root).unwrap_or_else(|d| panic!("{d:#?}"));
        let changes = generator.changes(&root, &generated).expect("readable output");
        assert!(changes.is_empty(), "{} is stale; run `{}`", generator.output, crate::RUN);
    }
}

#[test]
fn a_file_no_longer_generated_is_stale() {
    let root = vun_fs::Scratch::new("generate-stale");
    let output = root.join(generate::ALL[0].output);
    root.write(output.join("mod.rs"), "current");
    root.write(output.join("removed.rs"), "old");
    let generated = std::collections::BTreeMap::from([("mod.rs".to_owned(), "current".to_owned())]);
    let changes = generate::ALL[0].changes(&root.join(""), &generated).expect("readable scratch");
    assert!(changes.differing.is_empty());
    assert_eq!(changes.stale, [output.join("removed.rs")]);
}

#[test]
fn explanation_pages_are_named_by_binding_paths() {
    use std::path::Path;
    use vun_build::layout::Location;
    use vun_diag::DiagId;

    let page = crate::errors::Id::page(&["lang".to_owned(), "errors".to_owned()], &["int", "does_not_fit"]);
    assert_eq!(page, Path::new("lang/lang/errors/int/does_not_fit.md"));
    // A package's lint, `mydrivers.lints.raw_mmio`, by the same rule through its own layout.
    let lint = Location::at(Path::new("drivers"), &["mydrivers", "lints", "raw_mmio"]);
    assert_eq!(lint.with_extension(DiagId::PAGE), Path::new("drivers/mydrivers/lints/raw_mmio.md"));
}
