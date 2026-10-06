use std::io;

use vun_command::Outcome;
use vun_component::Component;
use vun_diag::{DiagCtxt, Diagnostic, Msg, errors};

use crate::{Remote, Server, Servers};

/// Serves, and fails as the client did.
struct Leaving;

/// Cannot talk to its client.
struct Cut;

impl Server for Leaving {
    fn name(&self) -> &'static str {
        "leaving"
    }

    fn serve(&self, diag: &mut DiagCtxt) -> io::Result<Outcome> {
        diag.error(Diagnostic::error(errors::CLI_UNAVAILABLE, Msg::new().text("nothing")));
        Ok(Outcome::Failure)
    }
}

impl Server for Cut {
    fn name(&self) -> &'static str {
        "cut"
    }

    fn serve(&self, _: &mut DiagCtxt) -> io::Result<Outcome> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "the client is gone"))
    }
}

macro_rules! remote {
    ($stub:expr) => {
        Remote::of(Component::within(vun_component::descriptor!(Servers, $stub)).unwrap()).unwrap()
    };
}

#[test]
fn a_session_ends_with_its_outcome_and_what_the_server_reported() {
    let remote = remote!(Leaving);
    assert_eq!(remote.name(), "leaving");
    let mut diag = DiagCtxt::new();
    assert_eq!(remote.serve(&mut diag).unwrap(), Outcome::Failure);
    assert_eq!(diag.first_error().unwrap().id, errors::CLI_UNAVAILABLE);
}

#[test]
fn a_session_that_cannot_talk_to_its_client_is_an_error() {
    let error = remote!(Cut).serve(&mut DiagCtxt::new()).unwrap_err();
    assert_eq!(error.to_string(), "the client is gone");
}
