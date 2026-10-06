//! The library's side: a backend answering the host's calls, its source lines asked of the host.

use vun_backend::{Backend, Emitted, Line, Lines, Request};
use vun_codec::Malformed;
use vun_component::{Asked, Interface, Kind};
use vun_diag::DiagCtxt;
use vun_intern::Symbol;
use vun_origin::OriginId;

use crate::{Answer, Ask, Call, Emit, Reply};

/// The interface of backends.
pub struct Backends;

impl Interface for Backends {
    const KIND: Kind = Kind("backend");
    type Service = dyn Backend + Send + Sync;
    type Call<'a> = Call<'a>;
    type Reply = Reply;
    type Ask = Ask;
    type Answer = Answer;

    fn serve(backend: &Self::Service, call: Call<'static>, ask: Asked<'_, Self>) -> Reply {
        match call {
            Call::Info => Reply::Info(*backend.info()),
            Call::Emit(emit) => {
                let Emit { unit, target, profile, listing } = *emit;
                let lines = Asking(ask);
                let request = Request { unit: &unit, target: &target, lines: &lines, profile, listing };
                let (mut emitted, mut diag) = (Emitted::default(), DiagCtxt::new());
                let failed = backend.emit(&request, &mut emitted, &mut diag).is_err();
                Reply::Emitted { emitted, diag, failed }
            }
            Call::Division(target) => Reply::Division(backend.division(&target)),
            Call::FrameSizes { unit, report } => Reply::FrameSizes(backend.frame_sizes(&unit, &report)),
        }
    }
}

/// The host's source lines, asked one by one.
struct Asking<'a>(Asked<'a, Backends>);

impl Asking<'_> {
    /// What the host answered, which it must have understood.
    fn ask<T>(&self, ask: Ask, read: impl FnOnce(Answer) -> Result<T, Malformed>) -> T {
        let question = format!("{ask:?}");
        read((self.0)(ask))
            .unwrap_or_else(|e| panic!("the host answered {question} with a wrong answer: {e}"))
    }
}

impl Lines for Asking<'_> {
    fn line(&self, origin: OriginId) -> Option<Line> {
        self.ask(Ask::Line(origin), Answer::line)
    }

    fn declaration(&self, origin: OriginId) -> Vec<Symbol> {
        self.ask(Ask::Declaration(origin), Answer::declaration)
    }

    fn inlined(&self, origin: OriginId) -> Option<(OriginId, OriginId)> {
        self.ask(Ask::Inlined(origin), Answer::inlined)
    }
}
