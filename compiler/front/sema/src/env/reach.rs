//! Where `.name` finds `name` (spec 01, section 4): through borrows, which have no members, and
//! through owners that lack it, to what they own.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

use super::Env;
use crate::member::{Found, MemberTable, Members};
use crate::ty::Ty;

/// A step `.` takes before reaching a member (spec 01, section 4): `.*` of a borrow, which has
/// no members of its own, or of an owner lacking the member, through its `#[kind.owns]` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Hop {
    Borrow,
    Owner(Symbol),
}

impl Env<'_, '_> {
    /// The steps `.name` takes from a value of type `ty`, and the type it finds `name` in: the
    /// last owner stepped through when the owners it lacks lead back to one already passed.
    #[must_use]
    pub fn reach(&self, mut ty: Ty, name: Symbol) -> (Vec<Hop>, Ty) {
        let (mut hops, mut owners) = (Vec::new(), Vec::new());
        loop {
            if let Some(pointee) = ty.borrowed() {
                hops.push(Hop::Borrow);
                ty = pointee;
                continue;
            }
            let members = self.cx.get::<Members>(&ty);
            let absent =
                members.as_deref().is_some_and(|m| matches!(m.find(name), Found::Absent | Found::Variant(_)));
            match members.as_deref().and_then(MemberTable::deref) {
                Some((field, target))
                    if absent && !owners.contains(&ty) && self.method(ty, name).is_none() =>
                {
                    owners.push(ty);
                    hops.push(Hop::Owner(field));
                    ty = target;
                }
                _ => return (hops, ty),
            }
        }
    }
}
