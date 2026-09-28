use vun_hash::Hash;

use super::{Index, Listing};
use crate::manifest::{Identity, Version};

#[test]
fn urls_follow_the_identity() {
    let index = Index { base: "https://index.example/" };
    let hal = Identity { id: 0xc0ff_ee01, major: 2 };
    assert_eq!(index.listing(hal), "https://index.example/00000000c0ffee01/2/list");
    assert_eq!(
        index.archive(hal, Version::parse("2.1.4").unwrap()),
        "https://index.example/00000000c0ffee01/2/2.1.4.tar.zst"
    );
}

#[test]
fn listings_read_back_and_name_the_newest() {
    let v = |s| Version::parse(s).unwrap();
    let listing = Listing { releases: vec![(v("2.0.0"), Hash::of(b"a")), (v("2.1.4"), Hash::of(b"b"))] };
    assert_eq!(Listing::parse(&listing.to_string()), Some(listing.clone()));
    assert_eq!(listing.newer(v("2.0.0")), Some((v("2.1.4"), Hash::of(b"b"))));
    assert_eq!(listing.newer(v("2.1.4")), None);
    assert_eq!(Listing::parse("2.0 blake3:x"), None);
}
