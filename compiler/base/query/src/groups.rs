//! Groups of inputs. A group is the inputs a query says belong together ([`Query::group`]): a
//! package's description and files, the target, the checks. A memo reads through the groups of
//! all the inputs it reads, and holds while the inputs of those groups do, whatever other
//! groups' inputs are set to or whether they are set at all. The inputs of a group are
//! fingerprinted as one, kept up to date as inputs are set.

use std::collections::BTreeSet;

use vun_codec::{Decode, Encode};
use vun_hash::Stable;
use vun_intern::{Internable, Interned, Pool};

use super::Query;
use super::db::Db;
use super::fingerprint::Fingerprint;
use super::memo::Source;

/// The identities of some groups, sorted.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub(crate) struct Groups(Vec<Fingerprint>);

static GROUPS: Pool<Groups> = Pool::new();

impl Internable for Groups {
    const POOL: &'static str = "groups";

    fn pool() -> &'static Pool<Self> {
        &GROUPS
    }
}

/// The groups a memo reads through, held once however many memos read through them.
pub(crate) type Reach = Interned<Groups>;

impl Groups {
    /// The group of the input `key` of `Q`.
    pub(crate) fn identity<Q: Query>(key: &Q::Key) -> Fingerprint {
        Fingerprint::of(&Q::group(key))
    }

    /// Reading nothing.
    pub(crate) fn none() -> Reach {
        GROUPS.intern(Self(Vec::new()))
    }

    /// Reading `group` alone.
    pub(crate) fn only(group: Fingerprint) -> Reach {
        GROUPS.intern(Self(vec![group]))
    }

    /// Reading through all of `reaches`.
    pub(crate) fn all(reaches: &[Reach]) -> Reach {
        match reaches {
            [] => Self::none(),
            [only] => *only,
            [first, rest @ ..] if rest.iter().all(|reach| reach == first) => *first,
            _ => GROUPS.intern(Self(
                reaches
                    .iter()
                    .flat_map(|reach| reach.0.iter().copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            )),
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = Fingerprint> + '_ {
        self.0.iter().copied()
    }
}

impl Db {
    /// The fingerprint of the inputs of all of `reach`, as they are set now.
    pub(crate) fn inputs_of(&self, reach: Reach) -> Fingerprint {
        let groups = self.groups.pin();
        reach.iter().fold(Fingerprint::default(), |all, group| {
            all.with(groups.get(&group).copied().unwrap_or_default())
        })
    }

    /// Accounts for an input of `group` set, now `after`, having been `before`: what an input
    /// adds to its group is its key and fingerprint, if it is set, whoever shares it: a memo read
    /// it unset, or as a value another database does not hold, holds only while it stays so.
    pub(crate) fn regroup<Q: Query>(
        &self,
        key: &Q::Key,
        before: Option<(Source, Fingerprint)>,
        after: (Source, Fingerprint),
    ) {
        let part = |(source, fingerprint): (Source, Fingerprint)| {
            matches!(source, Source::Set(_)).then(|| Fingerprint::of(&(Stable::of(key), fingerprint)))
        };
        let (out, into) = (before.and_then(part), part(after));
        if out != into {
            let change =
                |inputs: &Fingerprint| inputs.without(out.unwrap_or_default()).with(into.unwrap_or_default());
            self.groups.pin().update_or_insert(
                Groups::identity::<Q>(key),
                change,
                change(&Fingerprint::default()),
            );
        }
    }
}
