//! The compilation unit (DWARF 4, section 3.1): one entry naming the unit, its producer, its
//! line table and the range of its code, with the abbreviation that describes it.

use vun_object::{Contents, Object, Reloc, RelocKind, SectionId};

use vun_bytes::Out;

const VERSION: u16 = 4;
const ADDRESS: u8 = 8;
/// `DW_TAG_compile_unit`, without children.
const COMPILE_UNIT: u64 = 0x11;
const NO_CHILDREN: u8 = 0;
/// The unit's one abbreviation.
const ABBREVIATION: u64 = 1;
/// `DW_AT_*`, `DW_FORM_*`.
const PRODUCER: u64 = 0x25;
const NAME: u64 = 0x03;
const STMT_LIST: u64 = 0x10;
const LOW_PC: u64 = 0x11;
const HIGH_PC: u64 = 0x12;
const STRING: u64 = 0x08;
const SEC_OFFSET: u64 = 0x17;
const ADDR: u64 = 0x01;
/// A length from the low address, for the high one.
const DATA8: u64 = 0x07;
/// The attributes, in the order the entry holds their values.
const ATTRIBUTES: [(u64, u64); 5] =
    [(PRODUCER, STRING), (NAME, STRING), (STMT_LIST, SEC_OFFSET), (LOW_PC, ADDR), (HIGH_PC, DATA8)];

pub(crate) struct Unit<'a> {
    pub(crate) name: &'a str,
    pub(crate) producer: &'a str,
    pub(crate) text: SectionId,
    pub(crate) line: SectionId,
}

impl Unit<'_> {
    /// Adds `.debug_abbrev` and `.debug_info` to `object`.
    pub(crate) fn write(&self, object: &mut Object) {
        let mut abbrev = Out::default();
        abbrev.uleb(ABBREVIATION);
        abbrev.uleb(COMPILE_UNIT);
        abbrev.u8(NO_CHILDREN);
        for (attribute, form) in ATTRIBUTES {
            abbrev.uleb(attribute);
            abbrev.uleb(form);
        }
        abbrev.uleb(0);
        abbrev.uleb(0);
        abbrev.u8(0);
        let abbrevs = object.section(b".debug_abbrev", Contents::Debug);
        object.sections[abbrevs].bytes = abbrev.0;
        let size = object.sections[self.text].bytes.len() as u64;
        let [abbrevs, line, text] = [abbrevs, self.line, self.text].map(|s| object.section_symbol(s));
        let mut info = Out::default();
        let mut relocs = Vec::new();
        let mut at =
            |info: &Out, symbol, kind| relocs.push(Reloc { offset: info.here(), symbol, kind, addend: 0 });
        info.u32(0);
        info.u16(VERSION);
        at(&info, abbrevs, RelocKind::Abs32);
        info.u32(0);
        info.u8(ADDRESS);
        info.uleb(ABBREVIATION);
        info.str(self.producer.as_bytes());
        info.str(self.name.as_bytes());
        at(&info, line, RelocKind::Abs32);
        info.u32(0);
        at(&info, text, RelocKind::Abs64);
        info.u64(0);
        info.u64(size);
        info.patch_length(0);
        let section = object.section(b".debug_info", Contents::Debug);
        object.sections[section].bytes = info.0;
        object.sections[section].relocs = relocs;
    }
}
