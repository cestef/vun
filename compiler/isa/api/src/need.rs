//! What a processor may have of an instruction set beyond what every processor of it has: a
//! feature of the set (`lang.arch.<set>.Feature`, `lang.target.Spec.features`) or a
//! floating-point unit computing floats of a width (`lang.target.Spec.floats`). A form of a table
//! says what it needs (`lang.arch.Need`); a [`Processor`] says what it has, and the assemblers take
//! the forms it has.

use std::marker::PhantomData;

use vun_inline::Needed;
use vun_intern::Symbol;
use vun_target::Floats;

use crate::Width;

/// An enum of a table, generated with its variants in order and their names as the table writes
/// them, which targets name features by.
pub trait Enumerated: Copy + Eq + 'static {
    const ALL: &'static [Self];

    fn name(self) -> &'static str;
}

/// What a form, or a width of it, needs of a processor (`lang.arch.Need`): a feature `F` of the
/// set, or a floating-point unit computing floats of the width `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Need<F, Z> {
    Feature(F),
    Float(Z),
}

impl<F: Enumerated, Z: Width> Need<F, Z> {
    /// The need as every instruction set's are named, which diagnostics and targets read.
    #[must_use]
    pub fn needed(self) -> Needed {
        match self {
            Self::Feature(feature) => Needed::Feature(Symbol::intern(feature.name())),
            Self::Float(size) => Needed::Float(size.bits()),
        }
    }
}

/// What a processor has of the instruction set whose features are `F`: its features, and the
/// floats its floating-point unit computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Processor<F> {
    /// Bit `k` for `F::ALL[k]`.
    features: u64,
    floats: Floats,
    // Holds no feature: shared between threads whatever `F` is.
    set: PhantomData<fn() -> F>,
}

impl<F: Enumerated> Processor<F> {
    /// One having `features` and computing `floats`.
    pub fn new(features: impl IntoIterator<Item = F>, floats: Floats) -> Self {
        let features = features.into_iter().fold(0, |bits, feature| bits | Self::bit(feature));
        Self { features, floats, set: PhantomData }
    }

    /// The processor of a target: the features of the set among those it names, and the floats it
    /// computes.
    #[must_use]
    pub fn of(features: &[Symbol], floats: Floats) -> Self {
        let named = |feature: &&F| features.contains(&Symbol::intern(feature.name()));
        Self::new(F::ALL.iter().filter(named).copied(), floats)
    }

    /// One having every feature of the set and computing every float.
    #[must_use]
    pub fn every() -> Self {
        Self::new(F::ALL.iter().copied(), Floats::ALL)
    }

    #[must_use]
    pub fn has<Z: Width>(self, need: Need<F, Z>) -> bool {
        match need {
            Need::Feature(feature) => self.features & Self::bit(feature) != 0,
            Need::Float(size) => self.floats.computes(size.bits()),
        }
    }

    /// The first of `needs` it lacks.
    pub fn lacks<Z: Width>(self, needs: impl IntoIterator<Item = Need<F, Z>>) -> Option<Need<F, Z>> {
        needs.into_iter().find(|&need| !self.has(need))
    }

    /// The first of `forms`, each with what it needs, that it has; what the first needs that it
    /// lacks where it has none of them, and `None` where there are none.
    ///
    /// # Errors
    /// Returns that need.
    pub fn choose<T, Z: Width, N: IntoIterator<Item = Need<F, Z>>>(
        self,
        forms: impl IntoIterator<Item = (T, N)>,
    ) -> Result<Option<T>, Need<F, Z>> {
        let mut lacked = None;
        for (form, needs) in forms {
            match self.lacks(needs) {
                None => return Ok(Some(form)),
                Some(need) => {
                    lacked.get_or_insert(need);
                }
            }
        }
        lacked.map_or(Ok(None), Err)
    }

    fn bit(feature: F) -> u64 {
        F::ALL.iter().position(|&f| f == feature).map_or(0, |k| 1 << k)
    }
}
