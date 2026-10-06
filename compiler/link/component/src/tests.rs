use std::path::PathBuf;

use vun_component::Component;
use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_link::{LinkPlan, Linker, LinkerInfo, Output};
use vun_target::testing::target;
use vun_target::{Name, Readiness, Run, TargetSpec};

use crate::{Linkers, Remote};

static INFO: LinkerInfo = LinkerInfo {
    id: Name("stub"),
    formats: &[Name("elf")],
    regions: false,
    outputs: &[Output::Executable],
    readiness: Readiness::Ready,
    runs: &[Run::Program("ld"), Run::Compiler],
};

/// Links, and writes nothing.
struct Quiet;

/// Fails with a diagnostic of its own.
struct Failing;

macro_rules! stub {
    ($ty:ty, $link:expr) => {
        impl Linker for $ty {
            fn info(&self) -> &LinkerInfo {
                &INFO
            }

            fn link(&self, _: &LinkPlan, _: &TargetSpec, diag: &mut DiagCtxt) -> Result<(), ErrorGuaranteed> {
                $link(diag)
            }
        }
    };
}

stub!(Quiet, |_: &mut DiagCtxt| Ok(()));
stub!(Failing, |diag: &mut DiagCtxt| {
    Err(diag.error(Diagnostic::error(errors::LINK_UNDEFINED, Msg::new().text("main"))))
});

macro_rules! remote {
    ($stub:expr) => {
        Remote::of(Component::within(vun_component::descriptor!(Linkers, $stub)).unwrap()).unwrap()
    };
}

fn plan() -> LinkPlan {
    LinkPlan {
        inputs: vec![PathBuf::from("a.o")],
        output: PathBuf::from("a.out"),
        kind: Output::Executable,
        entry: None,
        exports: Vec::new(),
        regions: Vec::new(),
        sections: Vec::new(),
    }
}

#[test]
fn the_facts_and_programs_of_a_linker_cross_the_boundary() {
    let remote = remote!(Quiet);
    assert_eq!(remote.info(), &INFO);
    assert_eq!(remote.runs(&target()), INFO.runs);
}

#[test]
fn a_link_that_succeeds_or_fails_reports_as_the_linker_does() {
    let mut diag = DiagCtxt::new();
    assert!(remote!(Quiet).link(&plan(), &target(), &mut diag).is_ok());
    assert!(diag.failed().is_none());
    assert!(remote!(Failing).link(&plan(), &target(), &mut diag).is_err());
    assert_eq!(diag.first_error().unwrap().id, errors::LINK_UNDEFINED);
}
