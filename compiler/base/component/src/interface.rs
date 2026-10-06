//! A kind of component as typed requests: what the host calls, what the component asks back.

use vun_codec::{Decode, Encode};

use crate::Kind;

/// What a component of one kind answers (`Call`, which may borrow what the host has, with `Reply`) and may ask of the host while it
/// works (`Ask` with `Answer`), over a service that does the work, which a component linked in
/// is called as directly.
pub trait Interface: Sized + 'static {
    const KIND: Kind;
    type Service: ?Sized + Send + Sync + 'static;
    type Call<'a>: Encode + Decode;
    type Reply: Encode + Decode;
    type Ask: Encode + Decode;
    type Answer: Encode + Decode;

    /// Answers `call` from `service`, asking the host through `ask`.
    fn serve(service: &Self::Service, call: Self::Call<'static>, ask: Asked<'_, Self>) -> Self::Reply;
}

/// How a component asks the host.
pub type Asked<'a, I> = &'a dyn Fn(<I as Interface>::Ask) -> <I as Interface>::Answer;
