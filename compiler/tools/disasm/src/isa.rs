//! The instruction sets a disassembler reads: each decodes the instruction at the start of some
//! bytes, by an independent decoder, and says what machine of an ELF file it is for.

use std::num::NonZeroUsize;

use vun_elf::Machine;
use yaxpeax_arch::{Decoder, LengthedInstruction, U8Reader};

/// The instruction at the start of some bytes: how long it is, how it is written, and where a
/// direct branch goes, as the distance from the instruction's first byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub len: NonZeroUsize,
    pub text: String,
    pub target: Option<i64>,
}

/// An instruction set that can be read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isa {
    X86_64,
    Arm64,
}

impl Isa {
    /// Every instruction set, with the machine of an ELF file whose code it is.
    pub const ALL: &[(Self, Machine)] = &[(Self::X86_64, Machine::X86_64), (Self::Arm64, Machine::Aarch64)];

    /// The instruction set of the code of a file for `machine`, if one can be read.
    #[must_use]
    pub fn of(machine: Machine) -> Option<Self> {
        Self::ALL.iter().find(|&&(_, m)| m == machine).map(|&(isa, _)| isa)
    }

    /// The instruction at the start of `code`; none where those bytes are no instruction.
    #[must_use]
    pub fn decode(self, code: &[u8]) -> Option<Decoded> {
        let (len, text, target) = match self {
            Self::X86_64 => x86_64(code)?,
            Self::Arm64 => arm64(code)?,
        };
        Some(Decoded { len: NonZeroUsize::new(len)?, text, target })
    }
}

/// An x86-64 instruction: a call, jump or loop to a distance past its end goes there.
fn x86_64(code: &[u8]) -> Option<(usize, String, Option<i64>)> {
    use yaxpeax_x86::long_mode::{InstDecoder, Opcode, Operand};
    let inst = InstDecoder::default().decode(&mut U8Reader::new(code)).ok()?;
    let len = usize::try_from(inst.len().to_const()).ok()?;
    let transfers = matches!(
        inst.opcode(),
        Opcode::CALL
            | Opcode::JMP
            | Opcode::JO
            | Opcode::JNO
            | Opcode::JB
            | Opcode::JNB
            | Opcode::JZ
            | Opcode::JNZ
            | Opcode::JA
            | Opcode::JNA
            | Opcode::JS
            | Opcode::JNS
            | Opcode::JP
            | Opcode::JNP
            | Opcode::JL
            | Opcode::JGE
            | Opcode::JLE
            | Opcode::JG
            | Opcode::LOOP
            | Opcode::LOOPZ
            | Opcode::LOOPNZ
            | Opcode::JRCXZ
            | Opcode::JECXZ
    );
    let distance = match inst.operand(0) {
        Operand::ImmediateI8 { imm } => Some(i64::from(imm)),
        Operand::ImmediateI32 { imm } => Some(i64::from(imm)),
        _ => None,
    };
    let target = distance.filter(|_| transfers).and_then(|d| d.checked_add(i64::try_from(len).ok()?));
    Some((len, inst.to_string(), target))
}

/// An arm64 instruction: a branch, call, or test of a register to a place relative to it goes
/// there.
fn arm64(code: &[u8]) -> Option<(usize, String, Option<i64>)> {
    use yaxpeax_arm::armv8::a64::{InstDecoder, Opcode, Operand};
    let inst = InstDecoder::default().decode(&mut U8Reader::new(code.get(..4)?)).ok()?;
    let transfers = matches!(
        inst.opcode,
        Opcode::B | Opcode::BL | Opcode::Bcc(_) | Opcode::CBZ | Opcode::CBNZ | Opcode::TBZ | Opcode::TBNZ
    );
    let target = inst.operands.iter().find_map(|operand| match operand {
        Operand::PCOffset(distance) if transfers => Some(*distance),
        _ => None,
    });
    Some((4, inst.to_string(), target))
}
