//! Why a call of a binder gave no bindings, on the host.

use std::error::Error;
use std::fmt;

use vun_component::Fault;

use crate::Failure;

#[derive(Debug)]
pub enum BindError {
    /// The binder ran and said why nothing could be bound.
    Failed(Failure),
    /// The component serving the binder gave no reply.
    Component(Fault),
}

impl From<Failure> for BindError {
    fn from(failure: Failure) -> Self {
        Self::Failed(failure)
    }
}

impl From<Fault> for BindError {
    fn from(fault: Fault) -> Self {
        Self::Component(fault)
    }
}

impl From<BindError> for Failure {
    /// What a served binder tells the host: a fault of a component nested in it has no encoding, so
    /// the reader reports it as it words it.
    fn from(error: BindError) -> Self {
        match error {
            BindError::Failed(failure) => failure,
            BindError::Component(fault) => Self::Errors(vec![fault.to_string()]),
        }
    }
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(failure) => failure.fmt(f),
            Self::Component(_) => f.write_str("the binder component failed"),
        }
    }
}

impl Error for BindError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Failed(failure) => Some(failure),
            Self::Component(fault) => Some(fault),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(_) => f.write_str("the library the header is read through is missing"),
            Self::Unreadable => f.write_str("the header could not be opened"),
            Self::Errors(reports) => write!(f, "the header has {} errors", reports.len()),
        }
    }
}

impl Error for Failure {}
