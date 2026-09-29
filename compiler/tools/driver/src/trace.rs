//! How much of its code a session's programs describe in their table (spec 07, section 3):
//! `--trace` names an entry of `lang.trace.Detail`, else the profile chooses, the calls in a
//! debug build and the sites in a release one; a program of a plan says its own in
//! `Options.trace`. It is the query input `TraceOf`.

use vun_command::{Args, Param, Values};
use vun_query::Sharing;
use vun_sema::target::Variants;
use vun_sema::trace::TraceOf;
use vun_target::{Detail, Profile};

use crate::Session;

pub const TRACE: Param = Param::option("trace")
    .about("what a trap reports, as `lang.trace.Detail` names it: the calls in a debug build, the sites in a release one")
    .values(Values::Choice(|| Detail::VARIANTS.iter().map(|(item, _)| item.name()).collect()));

/// Applies `--trace`, or the detail of the session's profile, to the session.
pub fn configure(session: &mut Session, args: &Args) {
    let named = args.value(&TRACE).and_then(|name| {
        Detail::VARIANTS.iter().find(|(item, _)| item.name() == name).map(|&(_, detail)| detail)
    });
    let profiled = match session.codegen.profile {
        Profile::Debug => Detail::Calls,
        Profile::Release => Detail::Sites,
    };
    session.set_trace(named.unwrap_or(profiled));
}

impl Session {
    /// Sets what the session's programs describe in their table.
    pub fn set_trace(&mut self, detail: Detail) {
        // Shared memos are kept by the detail, which they may read.
        self.db.set_as::<TraceOf>(&(), detail, Sharing::Chosen);
    }

    /// What the session's programs describe in their table.
    #[must_use]
    pub fn trace(&self) -> Detail {
        self.db.get::<TraceOf>(&())
    }
}
