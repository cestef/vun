use std::path::PathBuf;

use vun_build::layout::MANIFEST;
use vun_build::manifest::DEFAULT_DEPS;
use vun_fs::Scratch;

use super::{Found, Library};

/// A toolchain directory holding `bin/vun`, and the default dependencies in `lib/` when `whole`.
fn toolchain(name: &str, whole: bool) -> (Scratch, PathBuf) {
    let root = Scratch::new(name);
    let exe = root.write("bin/vun", "");
    let deps = if whole { DEFAULT_DEPS } else { &DEFAULT_DEPS[1..] };
    for dep in deps {
        root.write(PathBuf::from(Library::DIR).join(dep).join(MANIFEST), "");
    }
    (root, exe)
}

#[test]
fn an_installed_toolchain_reads_its_own_library() {
    let (root, exe) = toolchain("installed", true);
    let library = Library::locate(None, Some(&exe));
    assert_eq!(library, Library { dir: root.join(Library::DIR), found: Found::Installed });
}

#[test]
fn a_library_missing_a_default_dependency_is_not_the_toolchains() {
    let (_root, exe) = toolchain("partial", false);
    assert_eq!(Library::locate(None, Some(&exe)).found, Found::Checkout);
}

#[test]
fn the_variable_names_the_library_over_the_toolchains() {
    let (_root, exe) = toolchain("named", true);
    let library = Library::locate(Some("/elsewhere".into()), Some(&exe));
    assert_eq!(library, Library { dir: "/elsewhere".into(), found: Found::Named });
}

#[test]
fn a_build_in_the_checkout_reads_the_checkouts_library() {
    let exe = std::env::current_exe().unwrap();
    let library = Library::locate(None, Some(&exe));
    assert_eq!(library.found, Found::Checkout);
    assert!(Library::holds_defaults(&library.dir));
}
