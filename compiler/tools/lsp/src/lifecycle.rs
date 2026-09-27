//! The protocol's lifecycle: `initialize` agrees on the encoding of positions and announces the
//! capabilities; `shutdown` stops answering; `exit` ends the server, failing without a
//! `shutdown` before it.

use vun_command::Outcome;
use vun_json::{Object, Value};

use crate::methods::Method;
use crate::position::Encoding;
use crate::server::{Server, State};

/// How the server names itself.
const NAME: &str = "vun";

impl Server<'_> {
    pub(crate) fn initialize(&mut self, params: &Value<'_>) -> Value<'static> {
        let offered = params.pointer("/capabilities/general/positionEncodings").map(Value::items);
        self.encoding = offered.map_or(Encoding::DEFAULT, Encoding::chosen);
        self.state = State::Running;
        let info = Object::new().with("name", NAME).with("version", env!("CARGO_PKG_VERSION"));
        Object::new()
            .with("capabilities", Method::capabilities(self.encoding))
            .with("serverInfo", info)
            .into()
    }

    pub(crate) fn shutdown(&mut self) {
        self.state = State::ShutDown;
    }

    pub(crate) fn exit(&mut self) {
        let outcome = if self.state == State::ShutDown { Outcome::Success } else { Outcome::Failure };
        self.state = State::Exited(outcome);
    }
}
