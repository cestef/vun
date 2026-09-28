//! Conventions as data: the rules of each, which this crate declares ([`crate::Rules`]), and the
//! registers they name, which each backend declares for its machine.

use crate::Rules;

/// A calling convention on one machine: its rules and the machine's registers it uses. `I` is a
/// general register, `F` a floating-point one.
#[derive(Clone, Copy, Debug)]
pub struct Convention<I: 'static, F: 'static> {
    pub rules: &'static Rules,
    pub registers: Registers<I, F>,
}

/// The registers a convention names, in the order it takes them. Under
/// [`crate::Floats::Backfill`] the floating-point ones are the narrowest, a wider float named by
/// the first of its run.
#[derive(Clone, Copy, Debug)]
pub struct Registers<I: 'static, F: 'static> {
    pub int_args: &'static [I],
    pub float_args: &'static [F],
    pub int_results: &'static [I],
    pub float_results: &'static [F],
    /// The register of its own the address of a result in memory travels in (`x8` under
    /// AAPCS64); without one it takes the first general argument register (System V, where the
    /// callee also gives it back as its result, and AAPCS).
    pub result_address: Option<I>,
    /// The general registers a call preserves; a call may overwrite every other.
    pub preserved: &'static [I],
    /// The general registers the platform keeps for itself, which hold no value.
    pub reserved: &'static [I],
}
