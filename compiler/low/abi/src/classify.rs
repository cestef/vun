//! The classifier: where a call of a signature passes each value under a convention.

use vun_lir::{AbiType, Conv, Record, Signature};

use crate::free::Free;
use crate::record::{Class, Piece};
use crate::{Carrier, Convention, Large, Loc, Part, Passing, Ret, Rules, Unpassable, Variadic};

impl<I: Copy, F: Copy> Passing<Carrier<I, F>> {
    /// Where a call of `signature` under `convention` passes its values and gets its result. The
    /// C convention follows the rules; Vun's passes each value in general registers, then on the
    /// stack, and a wide integer as the C convention does.
    ///
    /// # Errors
    /// Returns what the convention cannot pass: `f16`, a scalar wider than a register as one
    /// value, a record part of floats of an odd size, or anything to a naked function.
    ///
    /// # Panics
    /// If the convention declares too few result registers for a result of a scalar or its
    /// halves.
    pub fn of(signature: &Signature, convention: &Convention<I, F>) -> Result<Self, Unpassable> {
        let c = match signature.conv {
            Conv::C => true,
            Conv::Vun => false,
            // A naked function is entered as its code: nothing is passed, as nothing sets it up.
            Conv::Naked if signature.params.is_empty() && signature.result.is_none() => {
                return Ok(Self { params: Vec::new(), ret: Ret::None, stack: 0, floats: 0 });
            }
            Conv::Naked => return Err(Unpassable::Naked),
        };
        let rules = &convention.rules.calling(signature);
        // C's integers are at most two registers wide.
        let passed = signature.params.iter().chain(&signature.result);
        if let Some(&AbiType::Wide { limb, limbs, .. }) =
            passed.into_iter().find(|p| c && matches!(p, AbiType::Wide { limbs, .. } if *limbs > 2))
        {
            return Err(Unpassable::Integer(limb.bytes(rules.pointer_bits()) * u64::from(limbs)));
        }
        let registers = &convention.registers;
        let bits = rules.pointer_bits();
        let mut results = Free::new(rules, registers.int_results, registers.float_results, false);
        let ret = match &signature.result {
            Some(AbiType::Scalar(s) | AbiType::Int(s, _)) => {
                Ret::Reg(results.one(rules.class(*s)?, s.bytes(bits)).expect("a result register"))
            }
            Some(AbiType::Address(_)) => {
                Ret::Reg(results.one(Class::Int, rules.word).expect("a result register"))
            }
            &Some(AbiType::Wide { limb, limbs, .. }) => {
                let (bytes, pieces) = (limb.bytes(bits), Piece::limbs(limb.bytes(bits), limbs));
                let regs =
                    results.take(&pieces, bytes * u64::from(limbs)).expect("a result register per limb");
                Ret::Split(Part::of(regs, &pieces))
            }
            Some(AbiType::Record(r)) if c => {
                let pieces = rules.pieces(r, rules.returned)?;
                let parts =
                    pieces.and_then(|pieces| Some(Part::of(results.take(&pieces, r.align).ok()?, &pieces)));
                parts.map_or(Ret::Memory, Ret::Split)
            }
            // Vun's convention passes the address of a result in memory as its last value.
            None | Some(AbiType::Record(_) | AbiType::Memory { .. }) => Ret::None,
        };
        let mut free = Free::new(rules, registers.int_args, registers.float_args, c);
        // A result in memory's address, in its own register or the first argument register.
        let result_address = match (&ret, registers.result_address) {
            (Ret::Memory, Some(own)) => Some(Carrier::Int(own)),
            (Ret::Memory, None) => free.one(Class::Int, rules.word),
            _ => None,
        };
        let mut params = Vec::new();
        for (i, param) in signature.params.iter().enumerate() {
            if signature.variadic == Some(i) && rules.variadic == Variadic::Stack {
                free.spill();
            }
            let loc = match param {
                &AbiType::Wide { limb, limbs, .. } => {
                    params.extend(free.wide(limb.bytes(bits), limbs));
                    continue;
                }
                AbiType::Record(r) if c => {
                    Self::record(&mut free, r, rules.pieces(r, rules.in_registers)?, rules)
                }
                AbiType::Scalar(s) | AbiType::Int(s, _) => free.scalar(rules.class(*s)?, s.bytes(bits)),
                AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => {
                    free.scalar(Class::Int, rules.word)
                }
            };
            params.push(loc);
        }
        if signature.through() {
            params.push(match (result_address, &ret) {
                (Some(reg), _) => Loc::Reg(reg),
                (None, Ret::Split(_)) => Loc::Returned,
                (None, _) => free.scalar(Class::Int, rules.word),
            });
        }
        Ok(Self { params, ret, stack: free.stack_size(), floats: free.floats_used() })
    }

    /// Where a record passed by value under the C convention travels: in its pieces, else as the
    /// rules say of a large one.
    fn record(
        free: &mut Free<'_, I, F>,
        r: &Record,
        pieces: Option<Vec<Piece>>,
        rules: &Rules,
    ) -> Loc<Carrier<I, F>> {
        match (pieces, rules.large) {
            (Some(pieces), _) => free.record(r, &pieces),
            (None, Large::Stack) => Loc::Copied { offset: free.stack(r.size, r.align), size: r.size },
            (None, Large::Address) => {
                Loc::Indirect { size: r.size, at: Box::new(free.scalar(Class::Int, rules.word)) }
            }
        }
    }
}
