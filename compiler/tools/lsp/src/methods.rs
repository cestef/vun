//! The methods the server answers, one row each: its name, whether it is a request or a
//! notification, when in the lifecycle it is answered, what handles it, and the capability that
//! announces it. The capabilities announced at initialization are derived from the rows.

use vun_json::{Object, Value};

use crate::message::Failure;
use crate::position::Encoding;
use crate::server::Server;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Answered by a response.
    Request,
    /// Answered by nothing.
    Notification,
}

/// When in the lifecycle a method is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admitted {
    /// Before initialization and after shutdown too: the lifecycle's own methods.
    Always,
    /// Between initialization and shutdown.
    Running,
}

/// What the server announces it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Capability {
    /// Documents opened, changed as a whole, and closed.
    Sync,
    Hover,
    Definition,
}

/// The protocol's number for sending each change as the document's whole text.
const FULL: u8 = 1;

impl Capability {
    /// The member announcing it among the server's capabilities.
    fn member(self) -> (&'static str, Value<'static>) {
        match self {
            Self::Sync => {
                ("textDocumentSync", Object::new().with("openClose", true).with("change", FULL).into())
            }
            Self::Hover => ("hoverProvider", true.into()),
            Self::Definition => ("definitionProvider", true.into()),
        }
    }
}

/// How a method is handled: its result, `null` for a notification.
pub(crate) type Handle = for<'s> fn(&mut Server<'s>, &Value<'_>) -> Result<Value<'static>, Failure>;

pub(crate) struct Method {
    pub(crate) name: &'static str,
    pub(crate) kind: Kind,
    pub(crate) admitted: Admitted,
    pub(crate) handle: Handle,
    pub(crate) capability: Option<Capability>,
}

const fn method(name: &'static str, kind: Kind, admitted: Admitted, handle: Handle) -> Method {
    Method { name, kind, admitted, handle, capability: None }
}

const fn announced(name: &'static str, kind: Kind, handle: Handle, capability: Capability) -> Method {
    Method { name, kind, admitted: Admitted::Running, handle, capability: Some(capability) }
}

// A handler is written as a closure: a method of `Server<'_>` names one lifetime of it, and a
// handler takes any.
#[expect(clippy::redundant_closure_for_method_calls, reason = "the method is not general enough")]
pub(crate) const METHODS: &[Method] = &[
    method("initialize", Kind::Request, Admitted::Always, |s, p| Ok(s.initialize(p))),
    method("initialized", Kind::Notification, Admitted::Running, |_, _| Ok(Value::Null)),
    method("shutdown", Kind::Request, Admitted::Running, |s, _| {
        s.shutdown();
        Ok(Value::Null)
    }),
    method("exit", Kind::Notification, Admitted::Always, |s, _| {
        s.exit();
        Ok(Value::Null)
    }),
    announced("textDocument/didOpen", Kind::Notification, |s, p| s.opened(p), Capability::Sync),
    announced("textDocument/didChange", Kind::Notification, |s, p| s.changed(p), Capability::Sync),
    announced("textDocument/didClose", Kind::Notification, |s, p| s.closed_document(p), Capability::Sync),
    announced("textDocument/hover", Kind::Request, |s, p| s.hover(p), Capability::Hover),
    announced("textDocument/definition", Kind::Request, |s, p| s.definition(p), Capability::Definition),
];

impl Method {
    /// The row of the method `name`.
    pub(crate) fn named(name: &str) -> Option<&'static Self> {
        METHODS.iter().find(|m| m.name == name)
    }

    /// The server's capabilities: what its rows announce, and the encoding of positions.
    pub(crate) fn capabilities(encoding: Encoding) -> Value<'static> {
        let mut object = Object::new().with("positionEncoding", encoding.name());
        let mut announced: Vec<Capability> = Vec::new();
        for capability in METHODS.iter().filter_map(|m| m.capability) {
            if !announced.contains(&capability) {
                announced.push(capability);
                let (name, value) = capability.member();
                object.push(name, value);
            }
        }
        Value::Object(object)
    }
}
