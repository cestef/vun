//! The library's side: a backend answering the host's calls, its source lines asked of the host.

use vun_backend::{Assist, Assisted, Backend, Emitted, Line, Lines, Request};
use vun_codec::Malformed;
use vun_component::{Asked, Interface, Kind};
use vun_diag::DiagCtxt;
use vun_intern::Symbol;
use vun_lir::FuncId;
use vun_origin::OriginId;
use vun_target::TargetSpec;

use crate::memo::Memo;
use crate::{Answer, Ask, Call, Reply};

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
                let asking = Asking::new(ask);
                let request = emit.request(&asking);
                let (mut emitted, mut diag) = (Emitted::default(), DiagCtxt::new());
                let failed = backend.emit(&request, &mut emitted, &mut diag).is_err();
                Reply::Emitted { emitted, diag, failed }
            }
            Call::Division(target) => Reply::Division(backend.division(&target)),
            Call::FrameSizes { unit, report } => Reply::FrameSizes(backend.frame_sizes(&unit, &report)),
            Call::Assists(target) => Reply::Assists(backend.assist().is_some_and(|by| by.assists(&target))),
            Call::Assist { emit, ids } => {
                let asking = Asking::new(ask);
                let request = emit.request(&asking);
                Reply::Assisted(backend.assist().map(|by| by.compile(&request, &ids)))
            }
        }
    }
}

/// The host's source lines and assistance, asked one by one, a line once.
struct Asking<'a> {
    host: Asked<'a, Backends>,
    lines: Memo<Option<Line>>,
    declarations: Memo<Vec<Symbol>>,
    inlined: Memo<Option<(OriginId, OriginId)>>,
}

impl<'a> Asking<'a> {
    fn new(host: Asked<'a, Backends>) -> Self {
        Self { host, lines: Memo::default(), declarations: Memo::default(), inlined: Memo::default() }
    }

    /// What the host answered, which it must have understood.
    fn ask<T>(&self, ask: Ask, read: impl FnOnce(Answer) -> Result<T, Malformed>) -> T {
        let question = format!("{ask:?}");
        read((self.host)(ask))
            .unwrap_or_else(|e| panic!("the host answered {question} with a wrong answer: {e}"))
    }
}

impl Assist for Asking<'_> {
    fn assists(&self, target: &TargetSpec) -> bool {
        self.ask(Ask::Assists(Box::new(target.clone())), Answer::assists)
    }

    fn compile(&self, _: &Request<'_>, ids: &[FuncId]) -> Assisted {
        self.ask(Ask::Assist(ids.to_vec()), Answer::assisted)
    }
}

impl Lines for Asking<'_> {
    fn line(&self, origin: OriginId) -> Option<Line> {
        self.lines.get(origin, || self.ask(Ask::Line(origin), Answer::line))
    }

    fn declaration(&self, origin: OriginId) -> Vec<Symbol> {
        self.declarations.get(origin, || self.ask(Ask::Declaration(origin), Answer::declaration))
    }

    fn inlined(&self, origin: OriginId) -> Option<(OriginId, OriginId)> {
        self.inlined.get(origin, || self.ask(Ask::Inlined(origin), Answer::inlined))
    }
}
