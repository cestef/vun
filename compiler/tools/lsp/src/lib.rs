//! The language server (spec 13, section 11): an editor's client served over standard input and
//! output, from one driver session and the query engine `vun check` computes with. What the
//! editor holds of an open file is the file, saved or not; it is checked after each change, and
//! the diagnostics are published per version, hover and definition read at a position.

mod documents;
mod language;
mod lifecycle;
mod message;
mod methods;
mod position;
mod server;
#[cfg(test)]
mod tests;
mod transport;
mod uri;

use std::io::{self, BufRead, Write};
use std::sync::mpsc;

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
    let (sender, received) = mpsc::channel();
    std::thread::spawn(move || {
        let mut incoming = transport::Incoming(input);
        loop {
            let next = incoming.next();
            let last = !matches!(next, Ok(Some(_)));
            if sender.send(next).is_err() || last {
                break;
            }
        }
    });
    server::Server::new(session).run(&received, &mut transport::Outgoing(output))
}
