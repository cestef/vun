//! Choosing the target of a program (spec 13, section 7): the `lang.target.Spec` a path of the
//! package `targets` names, or the one `targets.hosts` gives the machine running the toolchain.
//! The driver says which machine that is, so the choice is a function of its key.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_intern::Symbol;
use vun_query::{Cx, Query};
use vun_target::{Host, TargetSpec};

use super::{Read, Unread};
use crate::env::{Env, Unevaluated};
use crate::ty::Value;

/// How a target is chosen.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Choice {
    /// The declaration at this path inside the package `targets`, such as `linux.x86_64`.
    Path(Vec<Symbol>),
    /// The row of `targets.hosts` for the machine of these names.
    Host { arch: Symbol, os: Symbol },
}

impl Choice {
    /// The machine running the toolchain, named as `targets.hosts` names it.
    #[must_use]
    pub fn host() -> Self {
        Self::Host { arch: Symbol::intern(std::env::consts::ARCH), os: Symbol::intern(std::env::consts::OS) }
    }
}

/// Why no target is chosen.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unchosen {
    /// The path, or `targets.hosts`, has no value.
    Unevaluated(Unevaluated),
    /// Its value is not a literal of its type.
    Unread(Unread),
    /// `targets.hosts` has no row for the machine.
    Unhosted { arch: Symbol, os: Symbol },
}

impl fmt::Display for Unchosen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unevaluated(why) => write!(f, "it has no value: {}", why.why()),
            Self::Unread(unread) => unread.fmt(f),
            Self::Unhosted { arch, os } => write!(f, "`targets.hosts` has no row for `{arch}` on `{os}`"),
        }
    }
}

impl std::error::Error for Unchosen {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unread(unread) => Some(unread),
            Self::Unevaluated(_) | Self::Unhosted { .. } => None,
        }
    }
}

/// The target a program rooted at a file builds for, chosen as the key says.
pub struct Chosen;

impl Query for Chosen {
    const NAME: &'static str = "chosen_target";
    type Key = (FileRef, Choice);
    type Value = Result<TargetSpec, Unchosen>;

    fn compute(cx: &Cx<'_>, (file, choice): &Self::Key) -> Self::Value {
        let env = Env::of(cx, file).ok_or(Unchosen::Unevaluated(Unevaluated::Absent))?;
        match choice {
            Choice::Path(path) => {
                let names = std::iter::once(vun_items::targets.symbol()).chain(path.iter().copied());
                let value = env.path(names).map_err(Unchosen::Unevaluated)?;
                TargetSpec::read(&value).map_err(Unchosen::Unread)
            }
            &Choice::Host { arch, os } => {
                let hosts = env.item(vun_items::targets::hosts).map_err(Unchosen::Unevaluated)?;
                let (Value::Tuple(rows) | Value::Array(rows)) = hosts else {
                    return Err(Unchosen::Unread(Unread::Record));
                };
                for row in &rows {
                    let host = Host::read(row).map_err(Unchosen::Unread)?;
                    if host.arch == arch && host.os == os {
                        return Ok(host.target);
                    }
                }
                Err(Unchosen::Unhosted { arch, os })
            }
        }
    }
}
