//! Machine code kept between builds (decision 669): code taken from what an earlier build kept
//! makes the object compiling it again makes.

use vun_driver::Codegen;
use vun_driver::Persistence;
use vun_driver::codegen::Listed;

use crate::common::Fixture;

#[test]
fn kept_code_makes_the_object_compiling_makes() {
    let fixture = Fixture::new("kept-code");
    let program = |status: u8| {
        format!(
            "start := std.os.start(main);\n\
             main := (sys: &mut std.os.System) -> std.os.Exit {{ x: std.os.Status = {status}; .code(x) }};\n"
        )
    };
    let path = fixture.root.write("app.vun", program(1));
    let objects = |persistence: Persistence| {
        let mut session = fixture.session();
        session.persistence = persistence;
        session.codegen = Codegen::through(vun_backend_native::INFO.id);
        let file = session.load(&path).unwrap();
        session.check(file);
        let Ok(coded) = session.code(file, Listed::No) else { panic!("{:#?}", session.diag) };
        coded.artifacts.into_iter().map(|a| a.bytes).collect::<Vec<_>>()
    };
    // The code is kept, then read back, then read back but for the function an edit changed.
    for status in [1, 1, 2] {
        fixture.root.write("app.vun", program(status));
        assert_eq!(objects(Persistence::On), objects(Persistence::Off), "exit status {status}");
    }
}
