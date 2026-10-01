//! What a reached `lang.panic.fail` says (spec 03, section 7).

use vun_codec::{Decode, Encode};
use vun_diag::Msg;
use vun_origin::OriginId;

use crate::Value;

/// What a reached `lang.panic.fail` says: a variant of `lang.panic.Reason`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Failure {
    Message(String),
    /// A condition that did not hold: its source, and where it is written.
    Assertion {
        source: String,
        at: OriginId,
    },
}

impl Failure {
    /// The failure a value of `lang.panic.Reason` names.
    #[must_use]
    pub fn of(reason: &Value) -> Option<Self> {
        let Value::Variant { name, payload, .. } = reason else { return None };
        match payload.as_slice() {
            [message] if *name == vun_items::lang::panic_Reason::message.symbol() => {
                Some(Self::Message(message.text()?))
            }
            [source, Value::Origin(at)] if *name == vun_items::lang::panic_Reason::assertion.symbol() => {
                Some(Self::Assertion { source: source.text()?, at: *at })
            }
            _ => None,
        }
    }

    /// Where it points: the condition an assertion names.
    #[must_use]
    pub fn place(&self) -> Option<OriginId> {
        match self {
            Self::Message(_) => None,
            Self::Assertion { at, .. } => Some(*at),
        }
    }

    /// What it says.
    #[must_use]
    pub fn msg(&self) -> Msg {
        match self {
            Self::Message(message) => Msg::new().text(message.clone()),
            Self::Assertion { source, .. } => Msg::new().text("assertion failed: ").code(source.clone()),
        }
    }
}
