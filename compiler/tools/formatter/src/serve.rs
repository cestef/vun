//! The library's side: a formatter answering the host's calls.

use vun_component::{Asked, Interface, Kind};

use crate::{Call, Formatter, Reply};

/// The interface of formatters.
pub struct Formatters;

impl Interface for Formatters {
    const KIND: Kind = Kind("formatter");
    type Service = dyn Formatter;
    type Call<'a> = Call<'a>;
    type Reply = Reply;
    type Ask = ();
    type Answer = ();

    fn serve(formatter: &Self::Service, call: Call<'static>, _: Asked<'_, Self>) -> Reply {
        match call {
            Call::Name => Reply::Name(formatter.name()),
            Call::Layout { text, width } => {
                let width = usize::try_from(width).unwrap_or(usize::MAX);
                Reply::Laid(formatter.layout(&text, width))
            }
        }
    }
}
