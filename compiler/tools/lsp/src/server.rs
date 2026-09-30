//! The server: the session it answers from, the documents the client holds open, and the loop
//! handling events, the messages the client sends and what the tasks the server started report,
//! and publishing diagnostics once those waiting are handled.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use vun_command::Outcome;
use vun_data::ToData;
use vun_data::Value;
use vun_data::json::Json;
use vun_driver::Session;
use vun_driver::testing::Outcome as Ended;
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

/// What the server's loop handles.
pub(crate) enum Event {
    /// A message the client sent: its text, `None` once the client closed the stream.
    Frame(Result<Option<String>, Unframed>),
    /// What a run of `vun.test` reported of the tests of the document at `uri`.
    Tested { uri: Uri, ran: Vec<vun_driver::testing::Ran> },
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
    /// How the tests of each document ended when last run, by name, until it changes.
    pub(crate) results: BTreeMap<Uri, BTreeMap<String, Ended>>,
    /// Where events are sent, and where the loop receives them.
    pub(crate) events: Sender<Event>,
    received: Receiver<Event>,
}

impl<'s> Server<'s> {
    pub(crate) fn new(session: &'s mut Session) -> Self {
        let (events, received) = mpsc::channel();
        Self {
            session,
            documents: BTreeMap::new(),
            encoding: PositionEncoding::DEFAULT,
            state: State::Starting,
            changed: BTreeSet::new(),
            outbox: Vec::new(),
            asked: 0,
            results: BTreeMap::new(),
            events,
            received,
        }
    }

    /// Serves the events it receives until the client exits or closes the stream: each is
    /// handled in order, a message answered, and once none waits, what changed is checked and
    /// published.
    pub(crate) fn run(&mut self, out: &mut transport::Outgoing<impl Write>) -> io::Result<Outcome> {
        while let Ok(first) = self.received.recv() {
            let mut next = Some(first);
            while let Some(event) = next {
                match event {
                    Event::Frame(frame) => {
                        if let Some(ended) = self.frame(frame, out)? {
                            return Ok(ended);
                        }
                    }
                    Event::Tested { uri, ran } => self.tested(uri, ran),
                }
                next = self.received.try_recv().ok();
            }
            self.publish();
            for message in self.outbox.drain(..) {
                out.send(&Json(&message.to_data()).to_string())?;
            }
        }
        Ok(self.closed())
    }

    /// Answers the message `frame`: how the server ends, once it does.
    fn frame(
        &mut self,
        frame: Result<Option<String>, Unframed>,
        out: &mut transport::Outgoing<impl Write>,
    ) -> io::Result<Option<Outcome>> {
        let reply = match frame {
            Ok(Some(text)) => self.handle(&text),
            Ok(None) => return Ok(Some(self.closed())),
            Err(unframed) => {
                let failure = Failure::new(ErrorCode::ParseError, Reason::Unframed(unframed));
                tracing::warn!(%failure, "unreadable message");
                Some(Message::response(Id::Null, Err(failure.response())))
            }
        };
        if let Some(reply) = reply {
            out.send(&Json(&reply.to_data()).to_string())?;
        }
        Ok(match self.state {
            State::Exited(outcome) => Some(outcome),
            _ => None,
        })
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
