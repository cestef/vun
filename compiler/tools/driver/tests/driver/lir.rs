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

#[test]
fn a_frame_holds_its_state_parameters_and_what_lives_across_a_yield() {
    let fixture = Fixture::new("lir-frames");
    let path = fixture.root.write(
        "p.vun",
        "\
start := std.os.start(main);
counter := (n: u8) -> std.iter.Iter(u8) { mut a: u8 = n; yield a; a ^= 1; yield a; };
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut total: u8 = 0;
    for x in counter(5) { total ^= x; }
    .code(total)
};
",
    );
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let program = session.program(file);
    let frame = program
        .functions
        .iter()
        .filter_map(|i| session.db.get::<vun_mir::Concrete>(i).ok())
        .flat_map(|body| body.locals.iter().filter_map(|(_, l)| l.ty).collect::<Vec<_>>())
        .find(|ty| matches!(&**ty, vun_sema::ty::TyKind::Frame(_)))
        .expect("main holds a frame");
    let shaped = session.db.get::<vun_lower::Frames>(&frame);
    let shape = shaped.as_ref().as_ref().unwrap();
    assert_eq!((shape.state, shape.points.len()), (vun_lir::Scalar::I8, 2));
    let mut offsets: Vec<u64> = shape.stored.iter().filter_map(|(_, o)| *o).collect();
    offsets.sort_unstable();
    assert_eq!(offsets, [1, 2]);
    assert_eq!((shape.layout.size, shape.layout.align), (3, 1));
}
