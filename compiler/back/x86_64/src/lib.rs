//! The x86-64 assembler. Every instruction the toolchain emits is declared once in `table`, as
//! its forms: the encoder picks the first form an instruction's operands fit, the printer writes
//! the same form's text, and callers ask which forms fit before choosing operands (an immediate,
//! memory, or a register), as TPDE's snippet encoders do. Code goes into a `vun_asm::Buffer`
//! through [`Emit`].

mod encode;
mod inst;
mod operand;
mod reg;
mod table;

#[cfg(test)]
mod tests;

pub use encode::Emit;
pub use inst::Inst;
pub use operand::{Base, Mem, Operand};
pub use reg::{Cc, Reg, Size, Xmm};
pub use table::{Form, Kind, Layout, Op, Prefix, W};

/// The instruction set, as `vun-asm` knows it.
#[derive(Clone, Copy, Debug)]
pub struct X86_64;

/// A function's code as it is emitted.
pub type Asm<S> = vun_asm::Buffer<X86_64, S>;

/// A reference to a label: 32 bits, from the end of the field, which ends the instruction.
#[derive(Clone, Copy, Debug)]
pub struct Rel32;

impl vun_asm::Isa for X86_64 {
    type Inst<S: Copy> = Inst<S>;
    type Fixup = Rel32;

    fn patch(_: Rel32, code: &mut [u8], at: u64, target: u64) -> Result<(), vun_asm::Unpatched> {
        let distance = i128::from(target) - i128::from(at + 4);
        let rel = i32::try_from(distance).map_err(|_| vun_asm::Unpatched::Range {
            at,
            distance: i64::try_from(distance).unwrap_or(i64::MAX),
        })?;
        let at = usize::try_from(at).expect("code fits memory");
        code[at..at + 4].copy_from_slice(&rel.to_le_bytes());
        Ok(())
    }
}
