//! The steps of a plan read as what they do: each a `std.build.Action`, read member by member
//! through the handles of `std.build`'s declarations, so the driver never names a field as text.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;
use vun_items::std::{
    build_Action as action, build_Dep as dep, build_Flashed as flashed, build_Needed as needed,
    build_Options as options, build_Program as program, build_Region as region, build_Step as step,
    build_Tested as tested, build_Use as used,
};
use vun_sema::target::{Declared, Members, Read, Unread};
use vun_sema_ty::Value;
use vun_target::{Check, Detail, Output};

/// What a step does.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Action {
    /// A dependency by its name in `package.deps`, and the parameters it is built with.
    Dep {
        name: Symbol,
        params: Value,
    },
    Program(Box<Options>),
    /// The program built at a step, written to its device.
    Flash {
        program: usize,
    },
    /// The tests of the program built at a step, run.
    TestOn {
        program: usize,
    },
    /// The lint levels, as recorded.
    Lints(Value),
}

/// A program's options (`std.build.Options`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Options {
    /// Its root file, relative to the directory of the `build.vun`.
    pub root: Symbol,
    pub target: Declared,
    pub deps: Vec<Use>,
    pub memory: Vec<Region>,
    pub output: Output,
    /// What its unsafe code is checked for at run time.
    pub checks: Vec<Check>,
    /// What its table describes, where it says.
    pub trace: Option<Detail>,
}

/// A dependency a program uses, by its name and the step that builds it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Use {
    pub name: Symbol,
    pub step: usize,
}

/// A region of a program's memory.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Region {
    pub name: Symbol,
    pub origin: u64,
    pub len: u64,
}

impl Read for Action {
    fn read(value: &Value) -> Result<Self, Unread> {
        let Value::Variant { name, payload, .. } = value else { return Err(Unread::Record) };
        let [payload] = payload.as_slice() else { return Err(Unread::Record) };
        let m = || Members::of(payload);
        if *name == action::dep.symbol() {
            let m = m()?;
            Ok(Self::Dep { name: m.name(needed::name)?, params: m.get(needed::params)?.clone() })
        } else if *name == action::program.symbol() {
            Options::read(payload).map(|o| Self::Program(Box::new(o)))
        } else if *name == action::flash.symbol() {
            Ok(Self::Flash { program: Built::read(m()?.get(flashed::program)?)?.0 })
        } else if *name == action::test_on.symbol() {
            Ok(Self::TestOn { program: Built::read(m()?.get(tested::program)?)?.0 })
        } else if *name == action::lints.symbol() {
            Ok(Self::Lints(payload.clone()))
        } else {
            Err(Unread::Record)
        }
    }
}

impl Read for Options {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self {
            root: m.string(options::root)?,
            target: m.read(options::target)?,
            deps: m.list(options::deps)?,
            memory: m.list(options::memory)?,
            output: m.read(options::output)?,
            checks: m.list(options::checks)?,
            trace: m.read(options::trace)?,
        })
    }
}

impl Read for Use {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        let built = Members::of(m.get(used::dep)?)?;
        Ok(Self { name: m.name(used::name)?, step: Step::read(built.get(dep::step)?)?.0 })
    }
}

impl Read for Region {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { name: m.name(region::name)?, origin: m.int(region::origin)?, len: m.int(region::len)? })
    }
}

/// A `std.build.Step`: the position of a step.
struct Step(usize);

impl Read for Step {
    fn read(value: &Value) -> Result<Self, Unread> {
        Members::of(value)?.int(step::index).map(Self)
    }
}

/// A `std.build.Program`: the step that builds it.
struct Built(usize);

impl Read for Built {
    fn read(value: &Value) -> Result<Self, Unread> {
        Step::read(Members::of(value)?.get(program::step)?).map(|s| Self(s.0))
    }
}
