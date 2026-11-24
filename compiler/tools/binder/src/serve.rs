//! The library's side: a binder answering the host's calls.

use vun_component::{Asked, Interface, Kind};

use crate::{Binder, Call, Failure, Reply};

/// The interface of binders.
pub struct Binders;

impl Interface for Binders {
    const KIND: Kind = Kind("binder");
    type Service = dyn Binder;
    type Call<'a> = Call;
    type Reply = Reply;
    type Ask = ();
    type Answer = ();

    fn serve(binder: &Self::Service, call: Call, _: Asked<'_, Self>) -> Reply {
        match call {
            Call::Name => Reply::Name(binder.name()),
            Call::Bind(request) => Reply::Bound(binder.bind(&request).map_err(Failure::from)),
        }
    }
}
