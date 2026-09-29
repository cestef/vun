//! Laying a file out: the file header and the program headers, each section's bytes where its
//! alignment puts them, a loaded one at an offset its address agrees with modulo its segment's
//! alignment, then the section headers; each segment placed over what it covers.

use vun_bytes::{Out, Strings};

use crate::headers::{FileHeader, ProgramHeader, SectionHeader};
use crate::record::Word;
use crate::segment::{Covers, Segment};
use crate::{CURRENT, Contents, File, LITTLE, MAGIC, Section, SegmentKind};

impl File {
    /// The file's bytes.
    ///
    /// # Panics
    /// Panics on an address, offset or size past the class's words, or past 65535 sections or
    /// segments.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let class = self.class;
        let mut table = Strings::default();
        let names: Vec<u32> = self.sections.iter().map(|s| table.add(&s.name)).collect();
        let table = table.into_bytes();
        let prefix = FileHeader::size(class) + self.segments.len() as u64 * ProgramHeader::size(class);
        let mut out = Out::default();
        out.zeros(usize::try_from(prefix).expect("headers fit memory"));
        let mut headers = vec![SectionHeader::default()];
        for (at, section) in self.sections.iter().enumerate() {
            let (bytes, size) = match &section.contents {
                Contents::Bytes(bytes) => (bytes.as_slice(), bytes.len() as u64),
                &Contents::Zeros(size) => (&[][..], size),
                Contents::Names => (table.as_slice(), table.len() as u64),
            };
            let offset = match section.contents {
                Contents::Zeros(_) => out.here(),
                _ => self.place(&mut out, at, section),
            };
            out.bytes(bytes);
            headers.push(SectionHeader {
                name: names[at],
                kind: section.kind.number(),
                flags: Word(section.flags.0),
                address: Word(section.address),
                offset: Word(offset),
                size: Word(size),
                link: section.link,
                info: section.info,
                align: Word(section.align),
                entsize: Word(section.entsize),
            });
        }
        let at = pad(&mut out, class.word());
        for header in &headers {
            header.put(class, &mut out);
        }
        let mut front = Out::default();
        self.header(at).put(class, &mut front);
        for header in self.program_headers(&headers, prefix) {
            header.put(class, &mut front);
        }
        out.0[..front.0.len()].copy_from_slice(&front.0);
        out.0
    }

    /// The file header, its section headers at `sections`.
    fn header(&self, sections: u64) -> FileHeader {
        let class = self.class;
        let count = |n: usize| u16::try_from(n).expect("fewer than 65536 entries");
        let size = |bytes: u64| u16::try_from(bytes).expect("headers are small");
        let mut ident = [0; 16];
        ident[..MAGIC.len()].copy_from_slice(&MAGIC);
        ident[MAGIC.len()..MAGIC.len() + 3].copy_from_slice(&[class.ident(), LITTLE, CURRENT]);
        let names = self.sections.iter().position(|s| s.contents == Contents::Names);
        FileHeader {
            ident,
            kind: self.kind.number(),
            machine: self.machine.number(),
            version: u32::from(CURRENT),
            entry: Word(self.entry),
            program_headers: Word(if self.segments.is_empty() { 0 } else { FileHeader::size(class) }),
            section_headers: Word(sections),
            flags: self.flags,
            header_size: size(FileHeader::size(class)),
            program_header_size: size(ProgramHeader::size(class)),
            program_header_count: count(self.segments.len()),
            section_header_size: size(SectionHeader::size(class)),
            section_header_count: count(self.sections.len() + 1),
            names: names.map_or(0, |at| count(at + 1)),
        }
    }

    /// Pads `out` to where `section`, `sections[at]`, starts: its alignment, and for a loaded one
    /// an offset its address agrees with modulo its segment's alignment.
    fn place(&self, out: &mut Out, at: usize, section: &Section) -> u64 {
        let offset = pad(out, section.align);
        let loading = self.segments.iter().find(|s| s.kind == SegmentKind::Load && s.covers.contains(at));
        let Some(align) = loading.map(|s| s.align).filter(|&align| align > 1) else { return offset };
        let skew = section.address.wrapping_sub(offset) % align;
        out.zeros(usize::try_from(skew).expect("an alignment fits memory"));
        offset + skew
    }

    /// The program headers of its segments, the sections laid out as `headers` say and the
    /// program headers ending at `prefix`.
    fn program_headers(&self, headers: &[SectionHeader], prefix: u64) -> Vec<ProgramHeader> {
        let placed: Vec<ProgramHeader> =
            self.segments.iter().map(|segment| self.placed(segment, headers, prefix)).collect();
        // The program headers are in memory where the segment loading the file header puts them.
        let loaded = self.segments.iter().zip(&placed).find_map(|(segment, placed)| {
            (segment.kind == SegmentKind::Load && segment.covers.holds_headers()).then_some(placed.address.0)
        });
        self.segments
            .iter()
            .zip(placed)
            .map(|(segment, placed)| match segment.covers {
                Covers::ProgramHeaders => {
                    let address = Word(loaded.map_or(0, |at| at + placed.offset.0));
                    ProgramHeader { address, physical: address, ..placed }
                }
                _ => placed,
            })
            .collect()
    }

    /// Where `segment` is, in the file and in memory.
    fn placed(&self, segment: &Segment, headers: &[SectionHeader], prefix: u64) -> ProgramHeader {
        let (offset, address, file_size, memory_size) = match &segment.covers {
            Covers::Nothing => (0, 0, 0, 0),
            Covers::ProgramHeaders => {
                let at = FileHeader::size(self.class);
                (at, 0, prefix - at, prefix - at)
            }
            Covers::Sections { headers: with_headers, sections } => {
                let covered = &headers[sections.start + 1..=sections.end];
                let offset = if *with_headers { 0 } else { covered.first().map_or(0, |h| h.offset.0) };
                let address = covered.first().map_or(0, |h| h.address.0 - (h.offset.0 - offset));
                let in_file = self.sections[sections.clone()]
                    .iter()
                    .zip(covered)
                    .filter(|(s, _)| !matches!(s.contents, Contents::Zeros(_)))
                    .map(|(_, h)| h.offset.0 + h.size.0);
                let file_end = in_file.max().unwrap_or(if *with_headers { prefix } else { offset });
                let in_memory = covered.iter().map(|h| h.address.0 + h.size.0);
                let memory_end = in_memory.max().unwrap_or(address + file_end - offset);
                (offset, address, file_end - offset, memory_end - address)
            }
        };
        ProgramHeader {
            kind: segment.kind.number(),
            flags: segment.flags.0,
            offset: Word(offset),
            address: Word(address),
            physical: Word(address),
            file_size: Word(file_size),
            memory_size: Word(memory_size),
            align: Word(segment.align),
        }
    }
}

impl Covers {
    /// Whether it covers `sections[at]`.
    fn contains(&self, at: usize) -> bool {
        matches!(self, Self::Sections { sections, .. } if sections.contains(&at))
    }

    /// Whether it covers the file header and the program headers.
    fn holds_headers(&self) -> bool {
        matches!(self, Self::Sections { headers: true, .. })
    }
}

/// Pads `out` to a multiple of `align`, giving where the next byte goes.
fn pad(out: &mut Out, align: u64) -> u64 {
    let at = out.here().next_multiple_of(align.max(1));
    out.zeros(usize::try_from(at - out.here()).expect("an alignment fits memory"));
    at
}
