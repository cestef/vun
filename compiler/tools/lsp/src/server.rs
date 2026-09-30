//! The server: the session it answers from, the documents the client holds open, and the loop
//! reading messages, answering each, and publishing diagnostics once those waiting are handled.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use vun_command::Outcome;
use vun_data::ToData;
use vun_data::Value;
use vun_data::json::Json;
use vun_driver::Session;
use vun_lsp_protocol::Request;
use vun_lsp_protocol::base::{PositionEncoding, TextDocumentIdentifier, Uri};
use vun_lsp_protocol::rpc::{ErrorCode, Id, Message};
use vun_span::FileId;
use vun_trace::{Activity, tracing};

use crate::failure::{Failure, Reason};
use crate::message::Incoming;
use crate::methods::{Admitted, Kind, Method};
use crate::position::Lines;
use crate::transport::{self, Unframed};

/// Where the protocol's lifecycle is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Before `initialize`.
    Starting,
    Running,
    /// After `shutdown`: only `exit` is answered.
    ShutDown,
    /// After `exit`, with the status it ends with.
    Exited(Outcome),
}

/// A document the client holds open: its file, the version of it the client last sent, and that
/// version's lines.
pub(crate) struct Document {
    pub(crate) path: PathBuf,
    pub(crate) version: i64,
    pub(crate) file: FileId,
    pub(crate) lines: Lines,
}

pub(crate) struct Server<'s> {
    pub(crate) session: &'s mut Session,
    pub(crate) documents: BTreeMap<Uri, Document>,
    pub(crate) encoding: PositionEncoding,
    pub(crate) state: State,
    /// The documents changed since their diagnostics were last published.
    pub(crate) changed: BTreeSet<Uri>,
    /// Notifications and requests to send once the messages waiting are handled.
    pub(crate) outbox: Vec<Message>,
    /// The requests the server sent, which number the next.
    pub(crate) asked: i64,
}

impl<'s> Server<'s> {
    pub(crate) fn new(session: &'s mut Session) -> Self {
        Self {
            session,
            documents: BTreeMap::new(),
            encoding: PositionEncoding::DEFAULT,
            state: State::Starting,
            changed: BTreeSet::new(),
            outbox: Vec::new(),
            asked: 0,
        }
    }

    /// Serves the messages `received` until the client exits or closes the stream: each is
    /// answered in order, and once none waits, what changed is checked and published.
    pub(crate) fn run(
        &mut self,
        received: &Receiver<Result<Option<String>, Unframed>>,
        out: &mut transport::Outgoing<impl Write>,
    ) -> io::Result<Outcome> {
        while let Ok(first) = received.recv() {
            for frame in std::iter::once(first).chain(received.try_iter()) {
                let reply = match frame {
                    Ok(Some(text)) => self.handle(&text),
                    Ok(None) => return Ok(self.closed()),
                    Err(unframed) => {
                        let failure = Failure::new(ErrorCode::ParseError, Reason::Unframed(unframed));
                        tracing::warn!(%failure, "unreadable message");
                        Some(Message::response(Id::Null, Err(failure.response())))
                    }
                };
                if let Some(reply) = reply {
                    out.send(&Json(&reply.to_data()).to_string())?;
                }
                if let State::Exited(outcome) = self.state {
                    return Ok(outcome);
                }
            }
            self.publish();
            for message in self.outbox.drain(..) {
                out.send(&Json(&message.to_data()).to_string())?;
            }
        }
        Ok(self.closed())
    }

    /// How the server ends when the client closes the stream: as `exit` would.
    fn closed(&self) -> Outcome {
        if self.state == State::ShutDown { Outcome::Success } else { Outcome::Failure }
    }

    /// Handles the message `text`: the response to a request, none for a notification.
    pub(crate) fn handle(&mut self, text: &str) -> Option<Message> {
        match Incoming::read(text) {
            Err(failure) => Some(Message::response(Id::Null, Err(failure.response()))),
            Ok(Incoming::Request { id, method, params }) => {
                let result = match Method::named(&method) {
                    Some(method) if method.kind == Kind::Request => self.handled(method, &params),
                    _ => Err(Failure::new(ErrorCode::MethodNotFound, Reason::Method)),
                };
                Some(Message::response(id, result.map_err(|f| f.response())))
            }
            Ok(Incoming::Notification { method, params }) => {
                // A notification the server has no row for is one it may ignore.
                let method = Method::named(&method).filter(|m| m.kind == Kind::Notification)?;
                if let Err(failure) = self.handled(method, &params) {
                    tracing::warn!(method = method.name, %failure, "notification not handled");
                }
                None
            }
            Ok(Incoming::Response) => None,
        }
    }

    /// `method` handled with `params`, where the lifecycle admits it: timed as a stage named by the
    /// method, and logged with how long it took.
    fn handled(&mut self, method: &Method, params: &Value<'_>) -> Result<Value<'static>, Failure> {
        let started = Instant::now();
        let timer = Activity::stage(method.name).start();
        let handled = self.admit(method).and_then(|()| (method.handle)(self, params));
        drop(timer);
        tracing::debug!(method = method.name, took = ?started.elapsed(), "handled");
        handled
    }

    /// Whether `method` is answered where the lifecycle is.
    fn admit(&self, method: &Method) -> Result<(), Failure> {
        match self.state {
            State::Starting if method.admitted == Admitted::Running => {
                Err(Failure::new(ErrorCode::ServerNotInitialized, Reason::Uninitialized))
            }
            State::ShutDown if method.admitted == Admitted::Running => {
                Err(Failure::new(ErrorCode::InvalidRequest, Reason::ShutDown))
            }
            _ => Ok(()),
        }
    }

    /// Queues the request `R` of `params` to the client; its answer is not waited for.
    pub(crate) fn ask<R: Request>(&mut self, params: &R::Params) {
        self.asked += 1;
        self.outbox.push(Message::request(Id::Number(self.asked), R::METHOD, params.to_data()));
    }

    /// The open document `document` names.
    pub(crate) fn document(&self, document: &TextDocumentIdentifier) -> Result<&Document, Failure> {
        self.documents.get(&document.uri).ok_or(Failure::new(ErrorCode::InvalidParams, Reason::Closed))
    }
}
