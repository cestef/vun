//! Leaks (spec 05, section 2): a linear place still `init` where it must have been consumed, at
//! a return, at the end of a binding's scope, or where ending a suspended frame stops it.

use vun_diag::{Msg, errors};
use vun_origin::OriginId;

use super::Analysis;
use super::states::{State, States};
use vun_mir::Local;

impl Analysis<'_> {
    /// Every linear place still `init` where the function stops: nothing is closed implicitly.
    pub(super) fn leaks(&mut self, states: &States, origin: OriginId, stop: Stop) {
        let leaked: Vec<Local> =
            self.linear.iter().copied().filter(|&l| states.locals[l] != State::Uninit).collect();
        for local in leaked {
            self.leak(local, origin, stop);
        }
    }

    pub(super) fn leak(&mut self, local: Local, origin: OriginId, stop: Stop) {
        let what = self.body.locals[local]
            .name
            .map_or_else(|| Msg::new().text("a linear value"), |n| Msg::new().code(n.as_str()));
        let msg = what.text(" is still held here, and linear values must be consumed ").text(stop.when());
        let help = Msg::new()
            .text("pass it to what consumes it, such as a close or a free, or ")
            .code("defer")
            .text(" that call where the value is made");
        self.advise(errors::PLACE_LINEAR_LEAK, origin, msg, Some(help));
    }
}

/// Where a place's value must be consumed by.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop {
    Return,
    /// The end of a binding's scope.
    Scope,
    /// The end of the frame while suspended at a `yield`, which runs only what is pending there.
    Ended,
}

impl Stop {
    fn when(self) -> &'static str {
        match self {
            Self::Return => "before returning",
            Self::Scope => "before its scope ends",
            Self::Ended => "or deferred before a `yield`, where ending the frame stops it",
        }
    }
}
