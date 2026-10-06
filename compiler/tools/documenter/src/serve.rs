//! The library's side: a documenter answering the host's calls, asking it what declares a type.

use vun_component::{Asked, Interface, Kind};

use crate::{Answer, Ask, Call, Documenter, Reply, Types};

/// The interface of documenters.
pub struct Documenters;

impl Interface for Documenters {
    const KIND: Kind = Kind("documenter");
    type Service = dyn Documenter;
    type Call<'a> = Call<'a>;
    type Reply = Reply;
    type Ask = Ask;
    type Answer = Answer;

    fn serve(documenter: &Self::Service, call: Call<'static>, ask: Asked<'_, Self>) -> Reply {
        match call {
            Call::Name => Reply::Name(documenter.name()),
            Call::Document(sources) => Reply::Documented(documenter.document(&sources, &Asking(ask))),
        }
    }
}

/// The host's knowledge of the declarations, asked one by one.
struct Asking<'a>(Asked<'a, Documenters>);

impl Types for Asking<'_> {
    fn is_type(&self, path: &[String], name: &str) -> bool {
        (self.0)(Ask::IsType { path: path.to_vec(), name: name.to_owned() }).is_type()
    }
}
