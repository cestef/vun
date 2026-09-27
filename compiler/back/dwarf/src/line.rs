//! The line table (DWARF 4, section 6.2): a header naming the files, then a sequence of rows per
//! function, from the address of its symbol to its end. Each row is written with the standard
//! opcodes, advancing the line and the address by what changed; a row that repeats the line
//! before it is left out.

use vun_object::{Contents, Object, Reloc, RelocKind, SectionId};

use super::{Lines, Row};
use vun_bytes::Out;

const VERSION: u16 = 4;
/// Every instruction is counted in bytes, one operation each, and starts a statement.
const MIN_INSTRUCTION: u8 = 1;
const MAX_OPERATIONS: u8 = 1;
const DEFAULT_IS_STMT: u8 = 1;
/// The special opcodes' parameters, as compilers commonly choose them; none is written, but a
/// reader needs them to know the standard opcodes' number.
const LINE_BASE: i8 = -5;
const LINE_RANGE: u8 = 14;
const OPCODE_BASE: u8 = 13;
/// The operands of each standard opcode, `DW_LNS_copy` on.
const STANDARD_LENGTHS: [u8; OPCODE_BASE as usize - 1] = [0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1];
/// `DW_LNS_*`.
const COPY: u8 = 1;
const ADVANCE_PC: u8 = 2;
const ADVANCE_LINE: u8 = 3;
const SET_FILE: u8 = 4;
/// `DW_LNE_*`, after the extended opcode's zero and length.
const EXTENDED: u8 = 0;
const END_SEQUENCE: u8 = 1;
const SET_ADDRESS: u8 = 2;
/// The bytes of an address.
const ADDRESS: u8 = 8;

/// Where a sequence stands: DWARF numbers files from 1 and lines start at 1.
struct State {
    file: u64,
    line: u32,
    offset: u64,
}

impl State {
    const START: Self = Self { file: 1, line: 1, offset: 0 };
}

impl Lines {
    /// Adds `.debug_line` to `object`.
    pub(crate) fn table(&self, object: &mut Object) -> SectionId {
        let mut out = Out::default();
        let mut relocs = Vec::new();
        out.u32(0);
        out.u16(VERSION);
        let header = out.here();
        out.u32(0);
        for byte in [
            MIN_INSTRUCTION,
            MAX_OPERATIONS,
            DEFAULT_IS_STMT,
            LINE_BASE.cast_unsigned(),
            LINE_RANGE,
            OPCODE_BASE,
        ] {
            out.u8(byte);
        }
        for length in STANDARD_LENGTHS {
            out.u8(length);
        }
        // No include directories: files are named whole.
        out.u8(0);
        for file in &self.files {
            out.str(file.as_os_str().as_encoded_bytes());
            // The directory, time of change and size, none given.
            for _ in 0..3 {
                out.uleb(0);
            }
        }
        out.u8(0);
        out.patch_length(header);
        for sequence in self.sequences.iter().filter(|s| !s.rows.is_empty()) {
            out.u8(EXTENDED);
            out.uleb(u64::from(ADDRESS) + 1);
            out.u8(SET_ADDRESS);
            relocs.push(Reloc {
                offset: out.here(),
                symbol: sequence.symbol,
                kind: RelocKind::Abs64,
                addend: 0,
            });
            out.u64(0);
            let mut state = State::START;
            let mut last = None;
            for &row in &sequence.rows {
                if last.is_some_and(|l: Row| (l.file, l.line) == (row.file, row.line)) {
                    continue;
                }
                last = Some(row);
                state.advance(&mut out, row);
            }
            out.u8(ADVANCE_PC);
            out.uleb(sequence.size - state.offset);
            out.u8(EXTENDED);
            out.uleb(1);
            out.u8(END_SEQUENCE);
        }
        out.patch_length(0);
        let section = object.section(b".debug_line", Contents::Debug);
        object.sections[section].bytes = out.0;
        object.sections[section].relocs = relocs;
        section
    }
}

impl State {
    /// Moves to `row` and appends it.
    fn advance(&mut self, out: &mut Out, row: Row) {
        let file = u64::from(row.file.0) + 1;
        if file != self.file {
            out.u8(SET_FILE);
            out.uleb(file);
            self.file = file;
        }
        if row.line != self.line {
            out.u8(ADVANCE_LINE);
            out.sleb(i64::from(row.line) - i64::from(self.line));
            self.line = row.line;
        }
        if row.offset != self.offset {
            out.u8(ADVANCE_PC);
            out.uleb(row.offset - self.offset);
            self.offset = row.offset;
        }
        out.u8(COPY);
    }
}
