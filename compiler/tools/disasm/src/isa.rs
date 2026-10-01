//! The instruction sets a disassembler reads: each decodes the instruction at the start of some
//! bytes, by an independent decoder.

use std::num::NonZeroUsize;

use yaxpeax_arch::{Decoder, LengthedInstruction, U8Reader};

/// The instruction at the start of some bytes: how long it is, how it is written, and the place
/// it refers to by its own position, as the distance from its first byte: where a direct branch
/// goes, or what an operand relative to the instruction pointer addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub len: NonZeroUsize,
    pub text: String,
    pub target: Option<i64>,
}

/// An instruction set that can be read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isa {
    X86,
    X86_64,
    Arm64,
}

impl Isa {
    /// The instruction at the start of `code`; none where those bytes are no instruction.
    #[must_use]
    pub fn decode(self, code: &[u8]) -> Option<Decoded> {
        let (len, text, target) = match self {
            Self::X86 => x86_32(code)?,
            Self::X86_64 => x86_64(code)?,
            Self::Arm64 => arm64(code)?,
        };
        Some(Decoded { len: NonZeroUsize::new(len)?, text, target })
    }
}

/// Reads an x86 instruction with the decoder of `$mode`, `long_mode` or `protected_mode`: a call,
/// jump or loop to a distance past its end goes there (`$extra` the loops of the mode alone), and
/// so does an operand relative to the instruction pointer, which `$relative` says a base register
/// is, and only 64-bit code has.
macro_rules! x86 {
    ($name:ident, $mode:ident, extra: [$($extra:ident),*], relative: |$base:ident| $relative:expr) => {
        fn $name(code: &[u8]) -> Option<(usize, String, Option<i64>)> {
            use yaxpeax_x86::$mode::{InstDecoder, Opcode, Operand};
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
                    | Opcode::JECXZ
                    $(| Opcode::$extra)*
            );
            let distance = (0..inst.operand_count()).find_map(|i| match inst.operand(i) {
                Operand::ImmediateI8 { imm } if transfers => Some(i64::from(imm)),
                Operand::ImmediateI32 { imm } if transfers => Some(i64::from(imm)),
                Operand::Disp { base: $base, disp } if $relative => Some(i64::from(disp)),
                _ => None,
            });
            let target = distance.and_then(|d| d.checked_add(i64::try_from(len).ok()?));
            Some((len, inst.to_string(), target))
        }
    };
}

x86!(x86_64, long_mode, extra: [JRCXZ], relative: |base| base == yaxpeax_x86::long_mode::RegSpec::rip());
x86!(x86_32, protected_mode, extra: [], relative: |_base| false);

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
