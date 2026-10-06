//! The library's side: a scheme answering the host's calls.

use std::error::Error;

use vun_component::{Asked, Cause, Interface, Kind};
use vun_fetch::Scheme;

use crate::{Call, Reply};

/// The interface of URL schemes.
pub struct Schemes;

impl Interface for Schemes {
    const KIND: Kind = Kind("scheme");
    type Service = dyn Scheme;
    type Call<'a> = Call;
    type Reply = Reply;
    type Ask = ();
    type Answer = ();

    fn serve(scheme: &Self::Service, call: Call, _: Asked<'_, Self>) -> Reply {
        match call {
            Call::Name => Reply::Name(Box::leak(scheme.name().to_owned().into_boxed_str())),
            Call::Read(uri) => match scheme.read(&uri) {
                Ok(body) => Reply::Body(body),
                Err(failure) => Reply::Failed(Cause::of(&failure as &(dyn Error + 'static))),
            },
        }
    }
}
