use std::path::Path;

use vun_fs::Scratch;
use vun_span::Version;

use super::{BIN, NAME, Toolchains};

/// A toolchain tree in `dir`: an executable and a library.
fn tree(dir: &Path) {
    std::fs::create_dir_all(dir.join(BIN)).unwrap();
    std::fs::write(dir.join(BIN).join(NAME), "vun").unwrap();
    std::fs::create_dir_all(dir.join("lib").join("std")).unwrap();
    std::fs::write(dir.join("lib").join("std").join("build.vun"), "").unwrap();
}

fn version(text: &str) -> Version {
    text.parse::<Version>().unwrap()
}

#[test]
fn installed_toolchains_are_listed_oldest_first_and_one_is_the_default() {
    let scratch = Scratch::new("toolchains-listed");
    let toolchains = Toolchains::at(&scratch.join("toolchains"), &scratch.join("bin").join(NAME));
    assert_eq!(toolchains.installed().unwrap(), []);
    for v in ["0.10.0", "0.2.1"] {
        let staged = toolchains.staging(version(v));
        tree(&staged);
        toolchains.install(version(v), &staged).unwrap();
    }
    assert_eq!(toolchains.installed().unwrap(), [version("0.2.1"), version("0.10.0")]);
    assert_eq!(toolchains.default(), None);
    toolchains.make_default(version("0.10.0")).unwrap();
    assert_eq!(toolchains.default(), Some(version("0.10.0")));
    toolchains.remove(version("0.2.1")).unwrap();
    assert_eq!(toolchains.installed().unwrap(), [version("0.10.0")]);
}

#[test]
fn a_toolchain_tree_hashes_by_its_contents() {
    let scratch = Scratch::new("toolchains-hash");
    tree(&scratch.join("a"));
    tree(&scratch.join("b"));
    std::fs::write(scratch.join("b").join("unrelated"), "not in a toolchain").unwrap();
    let hash = |dir| Toolchains::hash(&scratch.join(dir)).unwrap();
    assert_eq!(hash("a"), hash("b"));
    std::fs::write(scratch.join("b").join(BIN).join(NAME), "changed").unwrap();
    assert_ne!(hash("a"), hash("b"));
}
