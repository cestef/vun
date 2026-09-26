use std::path::PathBuf;

use vun_object::{Binding, Contents, Machine, Object, RelocKind, Symbol, SymbolId, SymbolKind};

use super::encode::Out;
use super::{Lines, Row, Sequence};

#[test]
fn leb128_encodes_as_dwarf_reads_it() {
    let mut out = Out::default();
    out.uleb(624_485);
    out.sleb(-123_456);
    out.sleb(63);
    out.sleb(-64);
    out.sleb(64);
    assert_eq!(out.0, [0xe5, 0x8e, 0x26, 0xc0, 0xbb, 0x78, 0x3f, 0x40, 0xc0, 0x00]);
}

/// An object with a `.text` of two functions, of 16 and 8 bytes.
fn object() -> (Object, [SymbolId; 2]) {
    let mut object = Object::new(Machine::X86_64);
    let text = object.section(b".text", Contents::Code);
    let mut symbol = |name: &[u8], bytes: &[u8]| {
        let value = object.append(text, bytes, 1);
        object.symbols.push(Symbol {
            name: name.to_vec(),
            section: Some(text),
            value,
            size: bytes.len() as u64,
            kind: SymbolKind::Func,
            binding: Binding::Global,
        })
    };
    let f = symbol(b"f", &[0x90; 16]);
    let g = symbol(b"g", &[0x90; 8]);
    (object, [f, g])
}

/// A sequence read back: its rows as (offset, file, line), and the address it ends at.
type Read = (Vec<(u64, u64, i64)>, u64);

/// Reads the rows of a line program back, from after its header: each sequence's rows as
/// (offset, file, line), with the address at the end of the sequence.
fn rows(program: &[u8]) -> Vec<Read> {
    let mut at = 0;
    let uleb = |at: &mut usize| {
        let (mut v, mut shift) = (0u64, 0);
        loop {
            let byte = program[*at];
            *at += 1;
            v |= u64::from(byte & 0x7f) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                return (v, shift);
            }
        }
    };
    let (mut sequences, mut rows) = (Vec::new(), Vec::new());
    let (mut offset, mut file, mut line) = (0, 1, 1);
    while at < program.len() {
        let op = program[at];
        at += 1;
        match op {
            0 => {
                let (length, _) = uleb(&mut at);
                if program[at] == 1 {
                    sequences.push((std::mem::take(&mut rows), offset));
                    (offset, file, line) = (0, 1, 1);
                }
                at += usize::try_from(length).unwrap();
            }
            1 => rows.push((offset, file, line)),
            2 => offset += uleb(&mut at).0,
            3 => {
                let (v, shift) = uleb(&mut at);
                line += (v.cast_signed() << (64 - shift)) >> (64 - shift);
            }
            4 => file = uleb(&mut at).0,
            other => panic!("opcode {other} is not written"),
        }
    }
    sequences
}

#[test]
fn a_line_table_holds_each_function_from_its_symbol_to_its_end() {
    let (mut object, [f, g]) = object();
    let mut lines = Lines::default();
    let (a, b) = (lines.file(PathBuf::from("a.vun")), lines.file(PathBuf::from("b.vun")));
    assert_eq!(lines.file(PathBuf::from("a.vun")), a, "a file is named once");
    let row = |offset, file, line| Row { offset, file, line };
    // The repeated line 3 is written once.
    lines.sequence(Sequence {
        symbol: f,
        size: 16,
        rows: vec![row(0, a, 2), row(4, a, 3), row(6, a, 3), row(9, b, 1)],
    });
    lines.sequence(Sequence { symbol: g, size: 8, rows: vec![row(0, a, 7)] });
    let text = object.sections.iter().next().unwrap().0;
    lines.write(&mut object, text, "a.vun", "vun");
    let (_, line) = object.sections.iter().find(|(_, s)| s.name == b".debug_line").unwrap();
    // After the unit's length and version, the header's length says where the program starts.
    let header = u32::from_le_bytes(line.bytes[6..10].try_into().unwrap()) as usize;
    let sequences = rows(&line.bytes[10 + header..]);
    assert_eq!(sequences, [(vec![(0, 1, 2), (4, 1, 3), (9, 2, 1)], 16), (vec![(0, 1, 7)], 8)]);
    let addresses: Vec<_> = line.relocs.iter().map(|r| (r.symbol, r.kind)).collect();
    assert_eq!(addresses, [(f, RelocKind::Abs64), (g, RelocKind::Abs64)]);
    let (_, info) = object.sections.iter().find(|(_, s)| s.name == b".debug_info").unwrap();
    let kinds: Vec<_> = info.relocs.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [RelocKind::Abs32, RelocKind::Abs32, RelocKind::Abs64]);
    assert!(object.write().len() > 64, "the object with its debugging sections is written");
}

#[test]
fn nothing_is_written_without_rows() {
    let (mut object, [f, _]) = object();
    let mut lines = Lines::default();
    lines.sequence(Sequence { symbol: f, size: 16, rows: Vec::new() });
    let text = object.sections.iter().next().unwrap().0;
    lines.write(&mut object, text, "a.vun", "vun");
    assert_eq!(object.sections.len(), 1);
}
