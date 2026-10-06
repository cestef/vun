//! The library's side: a linker answering the host's calls.

use vun_component::{Asked, Interface, Kind};
use vun_diag::DiagCtxt;
use vun_link::Linker;

use crate::{Call, Reply};

/// The interface of linkers.
pub struct Linkers;

impl Interface for Linkers {
    const KIND: Kind = Kind("linker");
    type Service = dyn Linker + Send + Sync;
    type Call<'a> = Call<'a>;
    type Reply = Reply;
    type Ask = ();
    type Answer = ();

    fn serve(linker: &Self::Service, call: Call<'static>, _: Asked<'_, Self>) -> Reply {
        match call {
            Call::Info => Reply::Info(*linker.info()),
            Call::Runs(target) => Reply::Runs(linker.runs(&target)),
            Call::Link { plan, target } => {
                let mut diag = DiagCtxt::new();
                let failed = linker.link(&plan, &target, &mut diag).is_err();
                Reply::Linked { diag, failed }
            }
        }
    }
}
