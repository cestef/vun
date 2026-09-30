//! Choosing the target of a program (spec 13, section 7): the `lang.target.Spec` a path of the
//! package `targets` names, or the first of `targets.hosts` for the machine running the
//! toolchain (`vun_target::Machine::RUNNING`), which is fixed for a build of the toolchain.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_items::Item;
use vun_query::{Cx, Query};
use vun_sema_ty::Value;
use vun_sema_ty::trap::Unevaluated;
use vun_target::Machine;

use super::{Declared, Read, TargetPath, Unread};
use crate::env::Env;

/// How a target is chosen.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Choice {
    /// The declaration at this path inside the package `targets`, such as `linux.x86_64.kernel`.
    Path(TargetPath),
    /// The first of `targets.hosts` for the machine running the toolchain.
    Host,
    /// A target read already, such as a program's in a package's plan.
    Given(Box<Declared>),
}

impl Choice {
    /// The target a handle of `vun_items::targets` names.
    #[must_use]
    pub fn of(target: Item) -> Self {
        Self::Path(TargetPath::of(target))
    }
}

/// Why no target is chosen.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unchosen {
    /// The path, or `targets.hosts`, has no value.
    Unevaluated(Unevaluated),
    /// Its value is not a literal of its type.
    Unread(Unread),
    /// `targets.hosts` has no target for the running machine.
    Unhosted,
}

impl fmt::Display for Unchosen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unevaluated(why) => write!(f, "it has no value: {}", why.why()),
            Self::Unread(unread) => unread.fmt(f),
            Self::Unhosted => {
                let Machine { arch, os } = Machine::RUNNING;
                write!(f, "`targets.hosts` has no target for `{arch}` on `{os}`")
            }
        }
    }
}

impl std::error::Error for Unchosen {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unread(unread) => Some(unread),
            Self::Unevaluated(_) | Self::Unhosted => None,
        }
    }
}

/// The target a program rooted at a file builds for, chosen as the key says.
pub struct Chosen;

impl Query for Chosen {
    const NAME: &'static str = "chosen_target";
    type Key = (FileRef, Choice);
    type Value = Result<Declared, Unchosen>;

    fn compute(cx: &Cx<'_>, (file, choice): &Self::Key) -> Self::Value {
        let env = Env::of(cx, file).ok_or(Unchosen::Unevaluated(Unevaluated::Absent))?;
        match choice {
            Choice::Path(path) => {
                let value = env.path(path.from_package()).map_err(Unchosen::Unevaluated)?;
                Declared::read(&value).map_err(Unchosen::Unread)
            }
            Choice::Host => {
                let hosts = env.item(vun_items::targets::hosts).map_err(Unchosen::Unevaluated)?;
                let (Value::Tuple(targets) | Value::Array(targets)) = hosts else {
                    return Err(Unchosen::Unread(Unread::Record));
                };
                for target in &targets {
                    let declared = Declared::read(target).map_err(Unchosen::Unread)?;
                    if Machine::RUNNING.is(&declared.spec) {
                        return Ok(declared);
                    }
                }
                Err(Unchosen::Unhosted)
            }
            Choice::Given(spec) => Ok((**spec).clone()),
        }
    }
}
