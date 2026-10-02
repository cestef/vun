//! The ELF reader accepts any bytes: it gives a file or says why not, copies no more of the bytes
//! than there are, and the file it gives reads on, to its symbols, relocations and code.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_elf::{File, Rela, SectionKind};

fuzz_target!(|bytes: &[u8]| {
    let Ok(file) = File::read(bytes) else { return };
    let copied: usize = file.sections.iter().map(|s| s.bytes().len()).sum();
    assert!(copied <= bytes.len(), "{copied} bytes copied out of {}", bytes.len());
    let _ = file.symbols();
    for section in file.sections.iter().filter(|s| s.kind == SectionKind::Rela) {
        let _ = Rela::read(file.class, section);
    }
    let _ = file.loaded(file.entry);
    if let Ok(code) = vun_disasm::Code::of_elf(&file) {
        let _ = vun_disasm::listing(&code, None);
    }
});
