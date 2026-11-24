use vun_hash::Hash;

use super::{Index, Listing, Misread, Reason};
use crate::manifest::{Identity, Version};

#[test]
fn urls_follow_the_identity() {
    let base = "https://index.example/".parse().unwrap();
    let index = Index { base: &base };
    let hal = Identity { id: 0xc0ff_ee01, major: 2 };
    assert_eq!(index.listing(hal).as_str(), "https://index.example/00000000c0ffee01/2/list");
    assert_eq!(
        index.archive(hal, "2.1.4".parse::<Version>().unwrap()).as_str(),
        "https://index.example/00000000c0ffee01/2/2.1.4.tar.zst"
    );
}

#[test]
fn listings_read_back_and_name_the_newest() {
    let v = |s: &str| s.parse::<Version>().unwrap();
    let listing = Listing { releases: vec![(v("2.0.0"), Hash::of(b"a")), (v("2.1.4"), Hash::of(b"b"))] };
    assert_eq!(listing.to_string().parse(), Ok(listing.clone()));
    assert_eq!(listing.newer(v("2.0.0")), Some((v("2.1.4"), Hash::of(b"b"))));
    assert_eq!(listing.newer(v("2.1.4")), None);
    assert_eq!("2.0 blake3:x".parse::<Listing>(), Err(Misread { line: 1, reason: Reason::Version }));
}
