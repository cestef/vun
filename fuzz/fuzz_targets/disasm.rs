//! Any bytes are listed, in each instruction set, to their end: every byte is in one instruction
//! or is data, and a file that starts as ELF or PE does is listed or refused.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_disasm::{Code, Isa, Line, Part, listing};

const ISAS: [Isa; 3] = [Isa::X86, Isa::X86_64, Isa::Arm64];

fuzz_target!(|input: (u8, &[u8])| {
    let (choice, bytes) = input;
    let _ = Code::read(bytes).and_then(|code| listing(&code, None));
    let isa = ISAS[usize::from(choice) % ISAS.len()];
    let part = Part::loaded(b".text", 0x1000, bytes.to_vec()).expect("a small run of code");
    let code = Code { isa, parts: vec![part], symbols: Vec::new(), linked: true };
    let lines = listing(&code, None).expect("no symbol is asked for");
    let covered: usize = lines
        .iter()
        .map(|line| match line {
            Line::Inst { bytes, .. } => bytes.len(),
            Line::Data { .. } => 1,
            Line::Section { .. } | Line::Label { .. } => 0,
        })
        .sum();
    assert_eq!(covered, bytes.len(), "{isa:?} lists every byte once");
});
