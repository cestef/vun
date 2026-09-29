//! The protocol's lifecycle: `initialize` agrees on the encoding of positions and announces the
//! capabilities; `shutdown` stops answering; `exit` ends the server, failing without a
//! `shutdown` before it.

use vun_command::Outcome;
use vun_json::Value;
use vun_lsp_protocol::base::PositionEncoding;
use vun_lsp_protocol::lifecycle::{InitializeParams, InitializeResult, ServerInfo};
use vun_lsp_protocol::{notifications as n, requests as r};

use crate::failure::Failure;
use crate::methods::{Answers, Hears, Method};
use crate::position::Counting;
use crate::server::{Server, State};

/// How the server names itself.
const NAME: &str = "vun";

impl Answers<r::Initialize> for Server<'_> {
    fn answer(&mut self, params: InitializeParams) -> Result<InitializeResult, Failure> {
        self.encoding = PositionEncoding::chosen(&params.encodings());
        self.state = State::Running;
        let mut capabilities = Method::capabilities();
        capabilities.position_encoding = Some(self.encoding);
        let info = ServerInfo { name: NAME.to_owned(), version: Some(env!("CARGO_PKG_VERSION").to_owned()) };
        Ok(InitializeResult { capabilities, server_info: Some(info) })
    }
}

impl Hears<n::Initialized> for Server<'_> {
    fn hear(&mut self, _: Value<'static>) -> Result<(), Failure> {
        Ok(())
    }
}

impl Answers<r::Shutdown> for Server<'_> {
    fn answer(&mut self, _: Option<Value<'static>>) -> Result<(), Failure> {
        self.state = State::ShutDown;
        Ok(())
    }
}

impl Hears<n::Exit> for Server<'_> {
    fn hear(&mut self, _: Option<Value<'static>>) -> Result<(), Failure> {
        let outcome = if self.state == State::ShutDown { Outcome::Success } else { Outcome::Failure };
        self.state = State::Exited(outcome);
        Ok(())
    }
}
