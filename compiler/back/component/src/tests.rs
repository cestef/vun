use std::path::PathBuf;

use vun_backend::{
    Artifact, ArtifactSink, Assist, Assisted, Backend, BackendCaps, BackendInfo, Compiled, Emits, Emitted,
    Feature, Line, Lines, Part, Request, Unassisted,
};
use vun_component::Component;
use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::unit::Unit;
use vun_lir::{Builder, Conv, FuncId, Linkage, Signature, Terminator};
use vun_lir_passes::legalize::{Division, Half};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};
use vun_target::testing::target;
use vun_target::{Name, Names, Profile, Readiness};

use crate::{Backends, Remote};

static INFO: BackendInfo = BackendInfo {
    id: Name("stub"),
    arches: Names::Only(&[Name("x86_64")]),
    formats: Names::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[],
        widest: vun_backend::Widest::Bits(64),
        half: Half::Promoted,
        floats: vun_backend::HeldFloats::Every,
    },
    readiness: Readiness::Ready,
    runs: &[],
};

/// Compiles any function as these bytes, for another backend.
struct Helper;

/// Emits what its unit's first function is compiled as by the assist of the request.
struct Needy;

/// Asks the line of its unit's first function three times.
struct Repeating;

/// Emits the line of its unit's first function, as the host knows it, as an artifact.
struct Located;

/// Fails with a diagnostic of its own.
struct Failing;

struct Panicking;

macro_rules! stub {
    ($ty:ty, $emit:expr) => {
        impl Backend for $ty {
            fn info(&self) -> &BackendInfo {
                &INFO
            }

            fn emit(
                &self,
                request: &Request<'_>,
                out: &mut dyn ArtifactSink,
                diag: &mut DiagCtxt,
            ) -> Result<(), ErrorGuaranteed> {
                $emit(request, out, diag)
            }

            fn division(&self, _: &vun_target::TargetSpec) -> Division {
                Division::Restoring
            }
        }
    };
}

stub!(Located, |request: &Request<'_>, out: &mut dyn ArtifactSink, _: &mut DiagCtxt| {
    let line = request.lines.line(request.unit.functions[FuncId::new(0)].origin).expect("the host knows it");
    let bytes = format!("{}:{}", line.file.display(), line.line).into_bytes();
    out.put(Artifact { part: Part::Whole, kind: Emits::Object, bytes });
    out.frames(vec![(FuncId::new(0), 48)]);
    Ok(())
});
stub!(Needy, |request: &Request<'_>, out: &mut dyn ArtifactSink, _: &mut DiagCtxt| {
    assert!(request.assist.assists(request.target));
    let compiled = request.assist.compile(request, &[FuncId::new(0)]).expect("assisted");
    out.put(Artifact { part: Part::Whole, kind: Emits::Object, bytes: compiled[0].1.code.bytes.clone() });
    Ok(())
});
stub!(Repeating, |request: &Request<'_>, _: &mut dyn ArtifactSink, _: &mut DiagCtxt| {
    let origin = request.unit.functions[FuncId::new(0)].origin;
    assert!((0..3).all(|_| request.lines.line(origin).is_some()));
    Ok(())
});
stub!(Failing, |_: &Request<'_>, _: &mut dyn ArtifactSink, diag: &mut DiagCtxt| {
    Err(diag.error(Diagnostic::error(errors::BACKEND_LACKS, Msg::new().text("no"))))
});
stub!(Panicking, |_: &Request<'_>,
                  _: &mut dyn ArtifactSink,
                  _: &mut DiagCtxt|
 -> Result<(), ErrorGuaranteed> { panic!("by request") });

static HELPER: BackendInfo =
    BackendInfo { caps: BackendCaps { features: &[Feature::Assist], ..INFO.caps }, ..INFO };

impl Backend for Helper {
    fn info(&self) -> &BackendInfo {
        &HELPER
    }

    fn emit(
        &self,
        _: &Request<'_>,
        _: &mut dyn ArtifactSink,
        _: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        Ok(())
    }

    fn assist(&self) -> Option<&dyn Assist> {
        Some(self)
    }
}

impl Assist for Helper {
    fn assists(&self, _: &vun_target::TargetSpec) -> bool {
        true
    }

    fn compile(&self, _: &Request<'_>, ids: &[FuncId]) -> Assisted {
        let compiled = |_| {
            let mut compiled = Compiled::unreached();
            compiled.code.bytes = vec![1, 2, 3];
            compiled
        };
        Ok(ids.iter().map(|&id| (id, compiled(id))).collect())
    }
}

/// Knows one origin's line, and counts the times it is asked.
struct Known(OriginId, std::cell::Cell<u32>);

impl Lines for Known {
    fn line(&self, origin: OriginId) -> Option<Line> {
        self.1.set(self.1.get() + 1);
        (origin == self.0).then(|| Line { file: PathBuf::from("a.vun"), line: 7, column: 1 })
    }
}

macro_rules! remote {
    ($stub:expr) => {
        Remote::of(Component::within(vun_component::descriptor!(Backends, $stub)).unwrap()).unwrap()
    };
}

/// An origin in the process's tables, which what crosses the boundary is read into.
fn at() -> OriginId {
    let file = SourceFiles::global().add("a.vun", "f := () {};");
    OriginTable::global().source(Span::new(file, 0, 1))
}

/// A unit of one function at `at`.
fn unit_at(at: OriginId) -> Unit {
    let signature =
        Signature { conv: Conv::Vun, params: Vec::new(), result: None, variadic: None, convention: None };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    b.terminate(Terminator::Return(None));
    let mut unit = Unit::default();
    unit.functions.push(b.finish());
    unit
}

#[test]
fn the_facts_of_a_backend_cross_the_boundary_whole() {
    let remote = remote!(Located);
    assert_eq!(remote.info(), &INFO);
    assert_eq!(remote.division(&target()), Division::Restoring);
}

#[test]
fn what_a_backend_emits_arrives_with_the_lines_it_asked_the_host() {
    let at = at();
    let unit = unit_at(at);
    let request = Request {
        unit: &unit,
        target: &target(),
        lines: &Known(at, std::cell::Cell::default()),
        assist: &Unassisted,
        profile: Profile::Debug,
        listing: false,
        kept: None,
    };
    let (mut emitted, mut diag) = (Emitted::default(), DiagCtxt::new());
    remote!(Located).emit(&request, &mut emitted, &mut diag).unwrap();
    assert_eq!(emitted.artifacts[0].bytes, b"a.vun:7");
    assert_eq!(emitted.frames, [(FuncId::new(0), 48)]);
}

/// What emitting through `remote` reports.
fn emitted_by(remote: &Remote) -> (Result<(), ErrorGuaranteed>, DiagCtxt) {
    let at = at();
    let unit = unit_at(at);
    let request = Request {
        unit: &unit,
        target: &target(),
        lines: &Known(at, std::cell::Cell::default()),
        assist: &Unassisted,
        profile: Profile::Debug,
        listing: false,
        kept: None,
    };
    let (mut emitted, mut diag) = (Emitted::default(), DiagCtxt::new());
    (remote.emit(&request, &mut emitted, &mut diag), diag)
}

#[test]
fn a_backend_that_fails_has_its_own_diagnostics_reported() {
    let (result, diag) = emitted_by(&remote!(Failing));
    assert!(result.is_err());
    assert_eq!(diag.first_error().unwrap().id, errors::BACKEND_LACKS);
}

#[test]
fn a_backend_that_panics_is_a_failed_component_that_can_be_called_again() {
    let remote = remote!(Panicking);
    for _ in 0..2 {
        let (result, diag) = emitted_by(&remote);
        assert!(result.is_err());
        assert_eq!(diag.first_error().unwrap().id, errors::COMPONENT_FAILED);
    }
}

#[test]
fn what_a_backend_cannot_express_is_compiled_by_the_assist_of_the_host() {
    let helper = remote!(Helper);
    assert!(helper.assist().is_some() && remote!(Needy).assist().is_none());
    let at = at();
    let unit = unit_at(at);
    let request = Request {
        unit: &unit,
        target: &target(),
        lines: &Known(at, std::cell::Cell::default()),
        assist: helper.assist().unwrap(),
        profile: Profile::Debug,
        listing: false,
        kept: None,
    };
    let (mut emitted, mut diag) = (Emitted::default(), DiagCtxt::new());
    remote!(Needy).emit(&request, &mut emitted, &mut diag).unwrap();
    assert_eq!(emitted.artifacts[0].bytes, [1, 2, 3]);
}

#[test]
fn a_line_the_host_gave_is_not_asked_again_in_the_same_call() {
    let at = at();
    let unit = unit_at(at);
    let known = Known(at, std::cell::Cell::default());
    let request = Request {
        unit: &unit,
        target: &target(),
        lines: &known,
        assist: &Unassisted,
        profile: Profile::Debug,
        listing: false,
        kept: None,
    };
    remote!(Repeating).emit(&request, &mut Emitted::default(), &mut DiagCtxt::new()).unwrap();
    assert_eq!(known.1.get(), 1);
}
