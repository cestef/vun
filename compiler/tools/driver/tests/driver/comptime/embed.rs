//! Files read at compile time (spec 03, section 7): `std.meta.embed` gives the bytes of a file of
//! the package, from the directory of the file calling, and traps for any other.

use crate::common::Fixture;
use vun_diag::errors;

/// The directory of data the package includes.
const DATA: &str = "data";

#[test]
fn a_file_of_the_package_is_embedded_as_its_bytes_and_any_other_traps() {
    let fixture = Fixture::new("comptime-embed");
    // A package holds what its manifest includes: its data directory among them.
    let mut manifest = Fixture::manifest("embedding", 1, "0.1.0");
    manifest.include.push(DATA.into());
    let dir = fixture.write(
        "embedding",
        &manifest,
        "\
table := std.meta.embed(\"data/table.txt\");
Len := Int(0..=table.len());
long: Len = 6;
First := Int(0..=table[0]);
first: First = 98;
Missing := Int(0..=std.meta.embed(\"data/absent.txt\").len());
missing: Missing = 0;
Outside := Int(0..=std.meta.embed(\"../outside.txt\").len());
outside: Outside = 0;
",
    );
    fixture.root.write(dir.join(DATA).join("table.txt"), "abcde");
    fixture.root.write("outside.txt", "not the package's");
    let reported: Vec<_> =
        fixture.reported(&Fixture::file(&dir, &manifest)).into_iter().map(|(id, _)| id).collect();
    let fit = errors::INT_DOES_NOT_FIT;
    assert_eq!(reported, [fit.clone(), fit, errors::COMPTIME_TRAP, errors::COMPTIME_TRAP]);
}
