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
