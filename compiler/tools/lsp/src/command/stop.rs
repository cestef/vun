//! Stopping runs: the programs `vun.run` started that have not ended, each ended by the command
//! `vun.stop`, and a lens beside the ones running them while any is.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::Child;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use vun_lsp_protocol::base::TextDocumentIdentifier;
use vun_lsp_protocol::command::CodeLens;
use vun_lsp_protocol::requests as r;
use vun_trace::tracing;

use super::Order;
use super::run::Ran;
use crate::server::{Event, Server};

const TITLE: &str = "Stop";

/// How often a run is looked at for having ended while it waits to be stopped.
const POLL: Duration = Duration::from_millis(25);

/// One run of a program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RunId(u64);

/// The runs that have not ended, by the way each is asked to stop.
#[derive(Default)]
pub(crate) struct Runs {
    next: u64,
    running: BTreeMap<RunId, Sender<()>>,
}

impl Runs {
    /// Whether any run has not ended.
    pub(crate) fn any(&self) -> bool {
        !self.running.is_empty()
    }

    /// Notes a run, which `stop` asks to end.
    fn begin(&mut self, stop: Sender<()>) -> RunId {
        let id = RunId(self.next);
        self.next += 1;
        self.running.insert(id, stop);
        id
    }

    /// Asks each run to end, and forgets those that cannot be asked.
    pub(crate) fn stop(&mut self) {
        self.running.retain(|_, stop| stop.send(()).is_ok());
    }

    /// Forgets the run `id`, which ended.
    pub(crate) fn end(&mut self, id: RunId) {
        self.running.remove(&id);
    }
}

impl Server<'_> {
    /// A lens stopping the runs, beside `run`, one that runs the program, while any runs.
    pub(super) fn stop_lens(&self, run: &CodeLens, document: &TextDocumentIdentifier) -> Option<CodeLens> {
        self.runs.any().then(|| {
            let ran = Ran { text_document: document.clone() };
            CodeLens { range: run.range, command: Some(Order::Stop.command(TITLE.to_owned(), &ran)) }
        })
    }

    /// Starts waiting for `child` to end, writing to `output` how it did, and ends it when the
    /// command `vun.stop` asks; the lenses show it as running meanwhile.
    pub(crate) fn follow(&mut self, mut child: Child, mut output: impl Write + Send + 'static) {
        let (stop, stopped) = mpsc::channel();
        let id = self.runs.begin(stop);
        let events = self.events.clone();
        std::thread::spawn(move || {
            let ended = loop {
                match stopped.recv_timeout(POLL) {
                    Ok(()) => {
                        let _ = child.kill();
                        break child.wait();
                    }
                    Err(RecvTimeoutError::Timeout) => match child.try_wait() {
                        Ok(Some(status)) => break Ok(status),
                        Ok(None) => {}
                        Err(error) => break Err(error),
                    },
                    // The server ended: the program is left to finish.
                    Err(RecvTimeoutError::Disconnected) => break child.wait(),
                }
            };
            if let Err(error) = ended.and_then(|status| writeln!(output, "\n{status}")) {
                tracing::warn!(%error, "a run did not end cleanly");
            }
            // The server may have ended since.
            drop(events.send(Event::Ended(id)));
        });
        self.ask::<r::CodeLensRefresh>(&None);
    }

    /// Ends the runs.
    pub(super) fn stop_runs(&mut self) {
        self.runs.stop();
    }

    /// Notes that the run `id` ended.
    pub(crate) fn ended(&mut self, id: RunId) {
        self.runs.end(id);
        self.ask::<r::CodeLensRefresh>(&None);
    }
}
