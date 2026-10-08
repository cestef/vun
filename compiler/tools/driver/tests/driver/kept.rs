//! Machine code kept between builds (decision 669): code taken from what an earlier build kept
//! makes the object compiling it again makes.

use vun_driver::Codegen;
use vun_driver::Persistence;
use vun_driver::codegen::Listed;

use crate::common::Fixture;

/// Builds `program` with each of `edits` in turn, kept and not, and compares the objects: the
/// code is kept, then read back, then read back but for the functions an edit changed.
fn same_objects(name: &str, program: impl Fn(u8) -> String, edits: &[u8]) {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("app.vun", program(edits[0]));
    let objects = |persistence: Persistence| {
        let mut session = fixture.session();
        session.persistence = persistence;
        session.codegen = Codegen::through(vun_backend_native::INFO.id);
        let file = session.load(&path).unwrap();
        session.check(file);
        let Ok(coded) = session.code(file, Listed::No) else { panic!("{:#?}", session.diag) };
        coded.artifacts.into_iter().map(|a| a.bytes).collect::<Vec<_>>()
    };
    for &edit in edits {
        fixture.root.write("app.vun", program(edit));
        assert_eq!(objects(Persistence::On), objects(Persistence::Off), "edit {edit}");
    }
}

#[test]
fn kept_code_makes_the_object_compiling_makes() {
    let program = |status: u8| {
        format!(
            "start := std.os.start(main);\n\
             main := (sys: &mut std.os.System) -> std.os.Exit {{ x: std.os.Status = {status}; .code(x) }};\n"
        )
    };
    same_objects("kept-code", program, &[1, 1, 2]);
}

/// Functions legalizing rewrites: one whose code is kept is not legalized again, and one calling
/// the division helper legalizing adds is compiled each time.
#[test]
fn kept_code_of_legalized_functions_makes_the_object_compiling_makes() {
    let program = |status: u8| {
        format!(
            "start := std.os.start(main);\n\
             scaled := (k: u8) -> u64 {{ wide: u128 = 0x1_0000_0000_0000_0000 * k + 7; (wide >> 60).wrap(u64) }};\n\
             thirds := (k: u8) -> u64 {{ wide: u128 = 0x1_0000_0000_0000_0000 * k + 7; (wide / 3).wrap(u64) }};\n\
             main := (sys: &mut std.os.System) -> std.os.Exit {{\n\
                 k: u8 = sys.args().len().sat(u8);\n\
                 mut failed: Int(0..=8) = {status};\n\
                 if (scaled(k) + thirds(k)).wrap(u64) == 7 {{ failed +|= 1; }}\n\
                 std.os.Exit.of(failed)\n\
             }};\n"
        )
    };
    same_objects("kept-legalized", program, &[1, 1, 2]);
}
