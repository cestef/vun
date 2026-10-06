//! Why a call of a component did not give a reply.

use std::any::Any;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use vun_codec::Malformed;
use vun_diag::{DiagCtxt, Diagnostic, ErrorGuaranteed, Msg, errors};

use crate::Kind;

#[derive(Debug)]
pub enum Fault {
    /// The library cannot be opened, or exports no component.
    Open { path: PathBuf, source: Box<dyn Error + Send + Sync> },
    /// It was built for another layout of the boundary.
    Abi { found: u32, expected: u32 },
    /// It was built in another release of the toolchain.
    Release { found: String, expected: &'static str },
    /// It is not the kind of component asked for.
    Kind { found: String, expected: Kind },
    /// No interface of this toolchain serves a component of this kind.
    Unserved(String),
    /// The component panicked, with this message.
    Panicked(String),
    /// What came back is not a reply.
    Malformed(Malformed),
    /// The component could not read the request, for this reason.
    Unread(String),
}

impl Fault {
    /// Reports that the component `name` failed, because of this.
    pub fn report(&self, name: &str, diag: &mut DiagCtxt) -> ErrorGuaranteed {
        let msg = Msg::new().text("component ").code(name.to_owned()).text(" failed");
        diag.error(Diagnostic::error(errors::COMPONENT_FAILED, msg).caused_by(self))
    }

    /// How a call the component said failed ends: with the error its own diagnostics made
    /// (`absorbed`, those it reported), or, when it said nothing, one saying so.
    ///
    /// # Errors
    /// Returns the proof of the error when `failed`.
    pub fn conclude(
        name: &str,
        failed: bool,
        absorbed: Option<ErrorGuaranteed>,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        match (failed, absorbed) {
            (false, _) => Ok(()),
            (true, Some(guarantee)) => Err(guarantee),
            (true, None) => {
                let msg =
                    Msg::new().text("component ").code(name.to_owned()).text(" failed without saying why");
                Err(diag.error(Diagnostic::error(errors::COMPONENT_FAILED, msg)))
            }
        }
    }

    /// The message a panic carried.
    #[must_use]
    pub fn text_of(payload: &(dyn Any + Send)) -> String {
        payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "without a message".to_owned())
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, .. } => write!(f, "{} is not a component that opens", path.display()),
            Self::Abi { found, expected } => {
                write!(f, "it speaks version {found} of the boundary, not {expected}")
            }
            Self::Release { found, expected } => write!(f, "it is of release {found}, not {expected}"),
            Self::Kind { found, expected } => write!(f, "it is of kind {found}, not {expected}"),
            Self::Unserved(kind) => write!(f, "this toolchain has no use for components of kind {kind}"),
            Self::Panicked(message) => write!(f, "it panicked: {message}"),
            Self::Malformed(_) => f.write_str("its reply is malformed"),
            Self::Unread(why) => write!(f, "it could not read the request: {why}"),
        }
    }
}

impl Error for Fault {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. } => Some(source.as_ref()),
            Self::Malformed(source) => Some(source),
            _ => None,
        }
    }
}
