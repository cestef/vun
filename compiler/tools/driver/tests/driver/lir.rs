//! Lowering programs to LIR (spec 13, section 3): what the exports reach becomes LIR that
//! verifies.

use vun_driver::Session;
use vun_harness::{ACCEPT, repo_root};
use vun_lir::{Conv, Linkage};

use crate::common::Fixture;

#[test]
fn a_hosted_program_lowers_to_lir_that_verifies() {
    let mut session = Session::new();
    let file = session.load(&repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap();
    assert_eq!(unit.verify(&session.origins), Ok(()));
    let main =
        unit.functions.iter().find(|(_, f)| matches!(f.linkage, Linkage::Export(s) if s.as_str() == "main"));
    assert_eq!(main.map(|(_, f)| f.signature.conv), Some(Conv::C));
}

#[test]
fn integers_arrays_and_runs_lower_with_their_checks() {
    let fixture = Fixture::new("lir-lowering");
    let path = fixture.root.write(
        "p.vun",
        "\
#[link.export(\"mix\"), abi.c]
mix := (x: u32, y: u32) -> u32 { (x ^ y) & 0xff };
#[link.export(\"item\")]
item := (xs: [4]u8, i: usize) -> u8 { xs[i] };
#[link.export(\"tail\")]
tail := (xs: &[u8], n: usize) -> usize { xs[n..].len() };
",
    );
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap();
    assert_eq!(unit.verify(&session.origins), Ok(()));
    // Indexing and slicing are trap sites.
    assert_eq!(unit.sites.len(), 2);
}
