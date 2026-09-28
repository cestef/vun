//! The x86-64 instruction set. Every instruction is declared once, in `lang.arch.x86_64`, as its
//! forms, from which `generated` is made: callers ask which forms fit before choosing operands (an
//! immediate, memory, or a register), as TPDE's snippet encoders do; the assembler
//! (`vun-asm-x86_64`) encodes the first form an instruction's operands fit, the listing writes
//! the same form's text, and sema checks blocks of inline assembly against the same forms.

mod fit;
#[rustfmt::skip]
mod generated;
mod inline;
mod inst;
mod operand;
mod reg;
mod table;

#[cfg(test)]
mod tests;

pub use inst::{Inst, Syntax, Text};
pub use operand::{Base, Mem, Operand};
pub use reg::{Cc, Class, Reg, Size, Xmm};
pub use table::{Access, Form, Kind, Layout, Op, Opcode, Prefix, Sizes, Slot, W};

/// The instruction set.
#[derive(Clone, Copy, Debug)]
pub struct X86_64;

/// A reference to a label: 32 bits, from the end of the field, which ends the instruction.
#[derive(Clone, Copy, Debug)]
pub struct Rel32;

impl vun_isa::Isa for X86_64 {
    type Inst<S: Copy> = Inst<S>;
    type Fixup = Rel32;

    fn patch(_: Rel32, code: &mut [u8], at: u64, target: u64) -> Result<(), vun_isa::Unpatched> {
        let distance = i128::from(target) - i128::from(at + 4);
        let rel = i32::try_from(distance).map_err(|_| vun_isa::Unpatched::Range {
            at,
            distance: i64::try_from(distance).unwrap_or(i64::MAX),
        })?;
        let at = usize::try_from(at).expect("code fits memory");
        code[at..at + 4].copy_from_slice(&rel.to_le_bytes());
        Ok(())
    }
}
