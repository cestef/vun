//! The language server (spec 13, section 11): an editor's client served over standard input and
//! output, from one driver session and the query engine `vun check` computes with. What the
//! editor holds of an open file is the file, saved or not; it is checked after each change, and
//! the diagnostics are published per version; hover, definition, references and rename, code
//! actions, signature help, inlay hints, completion, the outline, folds, formatting, semantic
//! tokens and views of each representation of a file are read from the same session.

mod assist;
mod command;
mod documents;
mod edit;
mod failure;
mod language;
mod lifecycle;
mod message;
mod methods;
mod outline;
mod position;
mod server;
#[cfg(test)]
mod tests;
mod tokens;
mod transport;

use std::io::{self, BufRead, Write};

use vun_command::Outcome;
use vun_driver::Session;

pub use transport::Unframed;

/// Serves the client over standard input and output until it exits: what `vun util lsp` runs.
///
/// # Errors
/// Returns why writing to the client failed.
pub fn serve(session: &mut Session) -> io::Result<Outcome> {
    serve_over(session, io::BufReader::new(io::stdin()), io::stdout().lock())
}

/// Serves the client writing `input` and reading `output`.
///
/// # Errors
/// Returns why writing to the client failed.
pub fn serve_over(
    session: &mut Session,
    input: impl BufRead + Send + 'static,
    output: impl Write,
) -> io::Result<Outcome> {
    // Messages are read as they come, so that those waiting when one is handled are known.
    let mut server = server::Server::new(session);
    let events = server.events.clone();
    std::thread::spawn(move || {
        let mut incoming = transport::Incoming(input);
        loop {
            let next = incoming.next();
            let last = !matches!(next, Ok(Some(_)));
            if events.send(server::Event::Frame(next)).is_err() || last {
                break;
            }
        }
        // The server keeps a sender of its own: the end of the stream is said, even after a
        // message that could not be read.
        let _ = events.send(server::Event::Frame(Ok(None)));
    });
    server.run(&mut transport::Outgoing(output))
}
