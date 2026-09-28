//! The classifier: where a call of a signature passes each value under a convention.

use vun_lir::{AbiType, Conv, Record, Signature};

use crate::convention::{Large, POINTER_BITS, WORD};
use crate::free::Free;
use crate::record::{Class, Piece};
use crate::{Carrier, Convention, Loc, Passing, Ret, Unpassable};

impl<I: Copy, F: Copy> Passing<Carrier<I, F>> {
    /// Where a call of `signature` under `convention` passes its values and gets its result. The
    /// C convention follows the rules; Vun's passes each value in general registers, then on the
    /// stack, and a wide integer as the C convention does.
    ///
    /// # Errors
    /// Returns what the convention cannot pass: `f16`, an unsplit 128-bit integer, a record part
    /// of floats of an odd size, or anything to a naked function.
    ///
    /// # Panics
    /// If the convention declares no result register of a class a scalar result takes, or the
    /// signature has a wide result, which the LIR verifier refuses.
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
        let (rules, registers) = (convention.rules, &convention.registers);
        let mut results = Free::new(rules, registers.int_results, registers.float_results, false);
        let ret = match &signature.result {
            Some(AbiType::Scalar(s)) => Ret::Reg(results.next(Class::of(*s)?).expect("a result register")),
            Some(AbiType::Int(..) | AbiType::Address(_)) => {
                Ret::Reg(results.next(Class::Int).expect("a result register"))
            }
            Some(AbiType::Record(r)) if c => {
                let parts = rules.pieces(r)?.and_then(|pieces| results.take(&pieces));
                parts.map_or(Ret::Memory, Ret::Split)
            }
            Some(AbiType::Wide { .. }) => unreachable!("the verifier refuses a wide result"),
            // Vun's convention passes the address of a result in memory as its last value.
            None | Some(AbiType::Record(_) | AbiType::Memory { .. }) => Ret::None,
        };
        let mut free = Free::new(rules, registers.int_args, registers.float_args, c);
        // A result in memory's address, in its own register or the first argument register.
        let result_address = match (&ret, registers.result_address) {
            (Ret::Memory, Some(own)) => Some(Carrier::Int(own)),
            (Ret::Memory, None) => free.next(Class::Int),
            _ => None,
        };
        let mut params = Vec::new();
        for param in &signature.params {
            let loc = match param {
                AbiType::Wide { .. } => {
                    params.extend(free.pair());
                    continue;
                }
                AbiType::Record(r) if c => Self::record(&mut free, r, rules.pieces(r)?, rules.large),
                AbiType::Scalar(s) => free.scalar(Class::of(*s)?, s.bytes(POINTER_BITS)),
                AbiType::Int(s, _) => free.scalar(Class::Int, s.bytes(POINTER_BITS)),
                AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => {
                    free.scalar(Class::Int, WORD)
                }
            };
            params.push(loc);
        }
        if signature.through() {
            params.push(match (result_address, &ret) {
                (Some(reg), _) => Loc::Reg(reg),
                (None, Ret::Split(_)) => Loc::Returned,
                (None, _) => free.scalar(Class::Int, WORD),
            });
        }
        Ok(Self { params, ret, stack: free.stack_size(), floats: free.floats_used() })
    }

    /// Where a record passed by value under the C convention travels: its pieces in registers,
    /// else copied onto the stack, or a large one as the rules say.
    fn record(
        free: &mut Free<'_, I, F>,
        r: &Record,
        pieces: Option<Vec<Piece>>,
        large: Large,
    ) -> Loc<Carrier<I, F>> {
        let Some(pieces) = pieces else {
            return match large {
                Large::Stack => Loc::Copied { offset: free.stack(r.size, r.align), size: r.size },
                Large::Address => Loc::Indirect { size: r.size, at: Box::new(free.scalar(Class::Int, WORD)) },
            };
        };
        match free.take(&pieces) {
            Some(parts) => Loc::Split(parts),
            None => Loc::Copied { offset: free.stack(r.size, r.align), size: r.size },
        }
    }
}
