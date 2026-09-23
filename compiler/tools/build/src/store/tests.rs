use std::path::Path;

use overt_hash::Hash;

use super::Store;

#[test]
fn paths_nest_by_algorithm_then_digest_prefix() {
    let hash = Hash::of(b"package");
    let digest = hash.digest();
    let store = Store::in_project(Path::new("p"));
    let entry = Path::new("p/.overt/store/blake3").join(&digest[..2]).join(&digest[2..]);
    assert_eq!(store.entry(hash), entry);
    assert_eq!(store.package(hash), entry.join("package"));
    assert_eq!(store.snapshot(hash), entry.join("snapshot"));
}
