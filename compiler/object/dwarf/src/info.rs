//! The compilation unit (DWARF 4, sections 3.1 and 3.3): one entry naming the unit, its producer,
//! its line table and the range of its code, holding an entry per function with its name, the
//! line declaring it and the range of its code, each described by an abbreviation.

use vun_object::{Contents, Object, Reloc, RelocKind, SectionId};

use vun_bytes::Out;

use super::Function;

const VERSION: u16 = 4;
/// `DW_TAG_*`.
const COMPILE_UNIT: u64 = 0x11;
const SUBPROGRAM: u64 = 0x2e;
/// `DW_CHILDREN_*`.
const NO_CHILDREN: u8 = 0;
const CHILDREN: u8 = 1;
/// `DW_AT_*`.
const PRODUCER: u64 = 0x25;
const NAME: u64 = 0x03;
const STMT_LIST: u64 = 0x10;
const LOW_PC: u64 = 0x11;
const HIGH_PC: u64 = 0x12;
const DECL_FILE: u64 = 0x3a;
const DECL_LINE: u64 = 0x3b;
/// `DW_FORM_*`.
const STRING: u64 = 0x08;
const SEC_OFFSET: u64 = 0x17;
const ADDR: u64 = 0x01;
const UDATA: u64 = 0x0f;
/// A length from the low address, for the high one.
const DATA8: u64 = 0x07;

/// An abbreviation: its code, the tag of its entries, whether they hold children, and their
/// attributes, in the order an entry holds their values.
struct Abbreviation {
    code: u64,
    tag: u64,
    children: u8,
    attributes: &'static [(u64, u64)],
}

const UNIT: Abbreviation = Abbreviation {
    code: 1,
    tag: COMPILE_UNIT,
    children: CHILDREN,
    attributes: &[
        (PRODUCER, STRING),
        (NAME, STRING),
        (STMT_LIST, SEC_OFFSET),
        (LOW_PC, ADDR),
        (HIGH_PC, DATA8),
    ],
};
const DECLARED: Abbreviation = Abbreviation {
    code: 2,
    tag: SUBPROGRAM,
    children: NO_CHILDREN,
    attributes: &[(NAME, STRING), (DECL_FILE, UDATA), (DECL_LINE, UDATA), (LOW_PC, ADDR), (HIGH_PC, DATA8)],
};
/// A function whose declaration no source names.
const UNDECLARED: Abbreviation = Abbreviation {
    code: 3,
    tag: SUBPROGRAM,
    children: NO_CHILDREN,
    attributes: &[(NAME, STRING), (LOW_PC, ADDR), (HIGH_PC, DATA8)],
};
const ABBREVIATIONS: [&Abbreviation; 3] = [&UNIT, &DECLARED, &UNDECLARED];

pub(crate) struct Unit<'a> {
    pub(crate) name: &'a str,
    pub(crate) producer: &'a str,
    pub(crate) text: SectionId,
    pub(crate) line: SectionId,
    pub(crate) functions: &'a [Function],
}

impl Unit<'_> {
    /// Adds `.debug_abbrev` and `.debug_info` to `object`.
    pub(crate) fn write(&self, object: &mut Object) {
        let mut abbrev = Out::default();
        for a in ABBREVIATIONS {
            abbrev.uleb(a.code);
            abbrev.uleb(a.tag);
            abbrev.u8(a.children);
            for &(attribute, form) in a.attributes {
                abbrev.uleb(attribute);
                abbrev.uleb(form);
            }
            abbrev.uleb(0);
            abbrev.uleb(0);
        }
        abbrev.u8(0);
        let abbrevs = object.section(b".debug_abbrev", Contents::Debug);
        object.sections[abbrevs].bytes = abbrev.0;
        let size = object.sections[self.text].bytes.len() as u64;
        let [abbrevs, line, text] = [abbrevs, self.line, self.text].map(|s| object.section_symbol(s));
        let (address, bytes) = (object.machine.address(), usize::from(object.machine.address_bytes()));
        let mut info = Out::default();
        let mut relocs = Vec::new();
        let mut at =
            |info: &Out, symbol, kind| relocs.push(Reloc { offset: info.here(), symbol, kind, addend: 0 });
        info.u32(0);
        info.u16(VERSION);
        at(&info, abbrevs, RelocKind::Abs32);
        info.u32(0);
        info.u8(object.machine.address_bytes());
        info.uleb(UNIT.code);
        info.str(self.producer.as_bytes());
        info.str(self.name.as_bytes());
        at(&info, line, RelocKind::Abs32);
        info.u32(0);
        at(&info, text, address);
        info.zeros(bytes);
        info.u64(size);
        for f in self.functions {
            info.uleb(f.declared.map_or(UNDECLARED.code, |_| DECLARED.code));
            info.str(f.name.as_bytes());
            if let Some(declared) = f.declared {
                info.uleb(declared.file.number());
                info.uleb(u64::from(declared.line));
            }
            at(&info, f.symbol, address);
            info.zeros(bytes);
            info.u64(f.size);
        }
        // The end of the unit's children.
        info.uleb(0);
        info.patch_length(0);
        let section = object.section(b".debug_info", Contents::Debug);
        object.sections[section].bytes = info.0;
        object.sections[section].relocs = relocs;
    }
}
