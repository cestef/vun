//! Call frame information (`.eh_frame`, DWARF 4 section 6.4 in the form the ELF ABIs load): how
//! to find, at every instruction of a function, the frame of its caller. Functions keep a frame
//! pointer, so a function's rows follow the steps of its prologue and epilogues alone: the frame
//! pointer and return address pushed, the frame pointer set, callee-saved registers saved, and
//! around each return the frame torn down.

use vun_object::{Contents, Object, Reloc, RelocKind, SymbolId};

use vun_bytes::Out;

/// A step of a function's frame, at the code offset it takes effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The frame pointer and the return address are stored: the caller's frame starts two
    /// words above the stack pointer.
    Pushed,
    /// The frame pointer is set: the caller's frame starts two words above it.
    Framed,
    /// The callee-saved register numbered `reg` is stored `offset` bytes from where the
    /// caller's frame starts.
    Saved { reg: u16, offset: i64 },
    /// An epilogue starts: the rows until `Returned` hold for it alone.
    Leaving,
    /// The frame pointer and return address are loaded back: the stack pointer is where it was
    /// at the call.
    Popped,
    /// The return is done: the rows before `Leaving` hold again.
    Returned,
}

/// A machine's call frames: the factors offsets are counted in and its registers' numbers.
#[derive(Clone, Copy, Debug)]
pub struct CallFrame {
    /// The bytes an instruction's offset counts in.
    pub code_align: u64,
    pub stack_pointer: u16,
    pub frame_pointer: u16,
    /// The column holding the return address.
    pub return_address: u16,
    /// Whether the call pushes the return address, one word, rather than leaving it in a
    /// register (a link register) that the prologue stores.
    pub return_pushed: bool,
}

/// One function's frame: its symbol, its size and its steps, in order.
pub struct Fde {
    pub symbol: SymbolId,
    pub size: u64,
    pub steps: Vec<(u64, Step)>,
}

const WORD: i64 = 8;
/// Offsets from where the caller's frame starts count in words, downwards.
const DATA_ALIGN: i64 = -WORD;
const CIE_VERSION: u8 = 1;
/// `z`: an augmentation length follows; `R`: then the encoding of an FDE's addresses.
const AUGMENTATION: &[u8] = b"zR";
/// `DW_EH_PE_pcrel | DW_EH_PE_sdata4`.
const PCREL_SDATA4: u8 = 0x1b;
/// `DW_CFA_*`.
const ADVANCE_LOC: u8 = 0x40;
const ADVANCE_LOC1: u8 = 0x02;
const ADVANCE_LOC2: u8 = 0x03;
const ADVANCE_LOC4: u8 = 0x04;
const OFFSET: u8 = 0x80;
/// The registers `DW_CFA_offset` numbers in its low six bits.
const OFFSET_REGS: u8 = 0x40;
const OFFSET_EXTENDED: u8 = 0x05;
const SAME_VALUE: u8 = 0x08;
const REMEMBER_STATE: u8 = 0x0a;
const RESTORE_STATE: u8 = 0x0b;
const DEF_CFA: u8 = 0x0c;
const DEF_CFA_REGISTER: u8 = 0x0d;
const DEF_CFA_OFFSET: u8 = 0x0e;
const NOP: u8 = 0;

impl CallFrame {
    /// Adds `.eh_frame` to `object`, one record per function of `fdes` after the common one.
    ///
    /// # Panics
    /// Past 4 GiB of frames or functions.
    pub fn write(&self, object: &mut Object, fdes: &[Fde]) {
        let mut out = Out::default();
        let mut relocs = Vec::new();
        self.cie(&mut out);
        for fde in fdes {
            let start = out.here();
            out.u32(0);
            out.u32(u32::try_from(out.here()).expect("frames fit 4 GiB"));
            relocs.push(Reloc { offset: out.here(), symbol: fde.symbol, kind: RelocKind::Pc32, addend: 0 });
            out.u32(0);
            out.u32(u32::try_from(fde.size).expect("functions fit 4 GiB"));
            out.uleb(0);
            let mut at = 0;
            for &(offset, step) in &fde.steps {
                self.advance(&mut out, offset - at);
                at = offset;
                self.step(&mut out, step);
            }
            Self::pad(&mut out, start);
        }
        let section = object.section(b".eh_frame", Contents::ReadOnly);
        object.sections[section].bytes = out.0;
        object.sections[section].relocs = relocs;
        object.sections[section].align = WORD.cast_unsigned();
    }

    /// The common record: at a call, the caller's frame starts where the stack pointer is,
    /// above the return address if the call pushed it.
    fn cie(&self, out: &mut Out) {
        out.u32(0);
        out.u32(0);
        out.u8(CIE_VERSION);
        out.str(AUGMENTATION);
        out.uleb(self.code_align);
        out.sleb(DATA_ALIGN);
        out.uleb(u64::from(self.return_address));
        out.uleb(1);
        out.u8(PCREL_SDATA4);
        out.u8(DEF_CFA);
        out.uleb(u64::from(self.stack_pointer));
        out.uleb(self.entry().cast_unsigned());
        if self.return_pushed {
            Self::saved(out, self.return_address, -WORD);
        }
        Self::pad(out, 0);
    }

    /// Where the caller's frame starts above the stack pointer at a call.
    fn entry(&self) -> i64 {
        if self.return_pushed { WORD } else { 0 }
    }

    fn step(&self, out: &mut Out, step: Step) {
        match step {
            Step::Pushed => {
                out.u8(DEF_CFA_OFFSET);
                out.uleb((2 * WORD).cast_unsigned());
                Self::saved(out, self.frame_pointer, -2 * WORD);
                if !self.return_pushed {
                    Self::saved(out, self.return_address, -WORD);
                }
            }
            Step::Framed => {
                out.u8(DEF_CFA_REGISTER);
                out.uleb(u64::from(self.frame_pointer));
            }
            Step::Saved { reg, offset } => Self::saved(out, reg, offset),
            Step::Leaving => out.u8(REMEMBER_STATE),
            Step::Popped => {
                out.u8(DEF_CFA);
                out.uleb(u64::from(self.stack_pointer));
                out.uleb(self.entry().cast_unsigned());
                let restored =
                    [Some(self.frame_pointer), (!self.return_pushed).then_some(self.return_address)];
                for reg in restored.into_iter().flatten() {
                    out.u8(SAME_VALUE);
                    out.uleb(u64::from(reg));
                }
            }
            Step::Returned => out.u8(RESTORE_STATE),
        }
    }

    /// `reg` is stored `offset` bytes from where the caller's frame starts.
    fn saved(out: &mut Out, reg: u16, offset: i64) {
        let factored = (offset / DATA_ALIGN).cast_unsigned();
        // The short form holds the register in six bits; others, such as arm64's `d8`, are
        // written in full.
        match u8::try_from(reg) {
            Ok(low) if low < OFFSET_REGS => out.u8(OFFSET | low),
            _ => {
                out.u8(OFFSET_EXTENDED);
                out.uleb(u64::from(reg));
            }
        }
        out.uleb(factored);
    }

    /// Moves the row `bytes` further into the function.
    fn advance(&self, out: &mut Out, bytes: u64) {
        let delta = bytes / self.code_align;
        match delta {
            0 => {}
            1..0x40 => out.u8(ADVANCE_LOC | u8::try_from(delta).expect("six bits")),
            0x40..0x100 => {
                out.u8(ADVANCE_LOC1);
                out.u8(u8::try_from(delta).expect("one byte"));
            }
            0x100..0x1_0000 => {
                out.u8(ADVANCE_LOC2);
                out.u16(u16::try_from(delta).expect("two bytes"));
            }
            _ => {
                out.u8(ADVANCE_LOC4);
                out.u32(u32::try_from(delta).expect("functions fit 4 GiB"));
            }
        }
    }

    /// Pads the record starting at `start` to a word with `DW_CFA_nop` and writes its length.
    fn pad(out: &mut Out, start: u64) {
        while !(out.here() - start).is_multiple_of(WORD.cast_unsigned()) {
            out.u8(NOP);
        }
        out.patch_length(start);
    }
}
