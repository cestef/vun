//! Any text compiled as a program's root file gives MIR and LIR that verify, or a reason it does
//! not lower, never a panic. One session serves every input, so the library is compiled once and
//! each input recomputes what its file reaches. Seed it with the programs of the corpus:
//! `cargo fuzz run program corpus/program ../corpus/accept/programs`
//! `../compiler/tools/driver/tests/driver/programs`.

#![no_main]

use std::cell::RefCell;

use libfuzzer_sys::fuzz_target;
use vun_diag::DiagCtxt;
use vun_driver::Session;
use vun_span::FileRef;

/// Where the input is, outside any package.
const PATH: &str = "/fuzz.vun";

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
}

fuzz_target!(|text: &str| {
    SESSION.with_borrow_mut(|session| {
        let file = session.sources.add_as(FileRef::loose(PATH), PATH, text);
        for (f, body) in session.bodies(file) {
            if let Ok(body) = body
                && let Err(malformed) = body.verify(session.origins)
            {
                panic!("the MIR of {f:?}: {malformed:?}");
            }
        }
        if let Ok(unit) = session.lir(file).as_ref()
            && let Err(malformed) = unit.verify(session.origins)
        {
            panic!("the LIR: {malformed:?}");
        }
        session.diag = DiagCtxt::default();
    });
});
