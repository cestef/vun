//! Linking (spec 13, section 7): a program linked by each linker that stands in for GNU ld runs
//! as it does linked by GNU ld; a linker is refused for a format it does not link.

use vun_diag::errors;
use vun_driver::Codegen;
use vun_intern::Symbol;
use vun_sema::target::Choice;
use vun_target::Name;
use vun_tool::Executable;

use crate::common::Fixture;

/// The program each linker links: it exits with 3.
const EXITS: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { .code(3) };
";

/// The linkers standing in for GNU ld, each with the program it runs, which the machine may lack.
const LINKERS: [(Name, &str); 2] = [(vun_link_mold::INFO.id, "mold"), (vun_link_wild::INFO.id, "wild")];

#[test]
fn a_program_runs_as_each_linker_links_it() {
    for (linker, program) in LINKERS {
        if Executable::search(program).is_none() {
            continue;
        }
        let fixture = Fixture::new(&format!("link-{linker}"));
        let path = fixture.root.write("main.vun", EXITS);
        let mut session = fixture.session();
        let file = session.load(&path).unwrap();
        session.codegen =
            Codegen { linker: Some(linker.symbol()), ..Codegen::through(vun_backend_native::INFO.id) };
        let built = session.build(file, None).unwrap_or_else(|_| panic!("{linker}: {:#?}", session.diag));
        let status = std::process::Command::new(built).status().unwrap();
        assert_eq!(status.code(), Some(3), "linked by {linker}");
    }
}

#[test]
fn a_linker_is_refused_for_a_format_it_does_not_link() {
    let fixture = Fixture::new("link-refused");
    let path = fixture.root.write("main.vun", EXITS);
    let mut session = fixture.session();
    session.choice = Choice::Path(["macos", "arm64"].map(Symbol::intern).to_vec());
    let file = session.load(&path).unwrap();
    session.codegen = Codegen {
        linker: Some(vun_link_mold::INFO.id.symbol()),
        ..Codegen::through(vun_backend_native::INFO.id)
    };
    assert!(session.build(file, None).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::LINK_UNSUPPORTED]);
}
