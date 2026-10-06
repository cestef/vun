//! The library's side: a server answering the host's calls.

use std::error::Error;

use vun_component::{Asked, Cause, Interface, Kind};
use vun_diag::DiagCtxt;

use crate::{Call, Reply, Server};

/// The interface of servers.
pub struct Servers;

impl Interface for Servers {
    const KIND: Kind = Kind("server");
    type Service = dyn Server;
    type Call<'a> = Call;
    type Reply = Reply;
    type Ask = ();
    type Answer = ();

    fn serve(server: &Self::Service, call: Call, _: Asked<'_, Self>) -> Reply {
        match call {
            Call::Name => Reply::Name(server.name()),
            Call::Serve => {
                let mut diag = DiagCtxt::new();
                match server.serve(&mut diag) {
                    Ok(outcome) => Reply::Served { outcome, diag },
                    Err(error) => Reply::Failed(Cause::of(&error as &(dyn Error + 'static))),
                }
            }
        }
    }
}
