//! Reading a file: its header, then its sections from their headers, named in the table the
//! header points to, then its segments, each found as what it covers.

use crate::headers::{FileHeader, ProgramHeader, SectionHeader};
use crate::input::{Input, Malformed};
use crate::segment::{Covers, Segment, SegmentFlags};
use crate::{
    Class, Contents, File, Kind, LITTLE, MAGIC, Machine, Section, SectionFlags, SectionKind, SegmentKind,
};

/// The names of a file without a table of them: the empty string alone.
const NAMELESS: &[u8] = &[0];

/// Where `EI_CLASS` and `EI_DATA` are in the file's identification.
const CLASS: usize = 4;
const DATA: usize = 5;

impl File {
    /// The file `bytes` hold.
    ///
    /// # Errors
    /// Where they are not a little-endian ELF file, or a header, a name or a section runs past
    /// their end.
    pub fn read(bytes: &[u8]) -> Result<Self, Malformed> {
        let ident: [u8; 16] = Input::at(bytes, 0).array()?;
        if ident[..MAGIC.len()] != MAGIC {
            return Err(Malformed::Magic);
        }
        let class = Class::of_ident(ident[CLASS]).ok_or(Malformed::Class(ident[CLASS]))?;
        if ident[DATA] != LITTLE {
            return Err(Malformed::BigEndian);
        }
        let header = FileHeader::take(class, &mut Input::at(bytes, 0))?;
        let mut input = Input::at(bytes, header.section_headers.0);
        let headers = (0..header.section_header_count)
            .map(|_| SectionHeader::take(class, &mut input))
            .collect::<Result<Vec<_>, _>>()?;
        // A file without a table of names names every section as the empty string.
        let names = match usize::from(header.names) {
            0 => NAMELESS,
            k => {
                let table = headers.get(k).ok_or(Malformed::Link(u32::from(header.names)))?;
                Input::at(bytes, table.offset.0).take(table.size.0)?
            }
        };
        let sections = headers
            .iter()
            .enumerate()
            .skip(1)
            .map(|(k, h)| Section::read(bytes, h, names, k == usize::from(header.names)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut input = Input::at(bytes, header.program_headers.0);
        let segments = (0..usize::from(header.program_header_count))
            .map(|k| {
                let placed = ProgramHeader::take(class, &mut input)?;
                Segment::read(k, &placed, &headers[1..], FileHeader::size(class))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            class,
            kind: Kind::of(header.kind),
            machine: Machine::of(header.machine),
            flags: header.flags,
            entry: header.entry.0,
            sections,
            segments,
        })
    }
}

impl Section {
    /// The section `header` describes, named in `names`; `holds_names` where it is their table.
    fn read(
        bytes: &[u8],
        header: &SectionHeader,
        names: &[u8],
        holds_names: bool,
    ) -> Result<Self, Malformed> {
        let kind = SectionKind::of(header.kind);
        let contents = if holds_names {
            Contents::Names
        } else if kind == SectionKind::NoBits {
            Contents::Zeros(header.size.0)
        } else {
            Contents::Bytes(Input::at(bytes, header.offset.0).take(header.size.0)?.to_vec())
        };
        Ok(Self {
            name: Input::string(names, u64::from(header.name))?.to_vec(),
            kind,
            flags: SectionFlags(header.flags.0),
            address: header.address.0,
            contents,
            link: header.link,
            info: header.info,
            align: header.align.0,
            entsize: header.entsize.0,
        })
    }
}

impl Segment {
    /// The `k`-th segment, `placed` over the sections `headers` describe, the file header
    /// `header_size` bytes.
    fn read(
        k: usize,
        placed: &ProgramHeader,
        headers: &[SectionHeader],
        header_size: u64,
    ) -> Result<Self, Malformed> {
        let kind = SegmentKind::of(placed.kind);
        let covers = if kind == SegmentKind::Phdr {
            Covers::ProgramHeaders
        } else if placed.file_size.0 == 0 && placed.memory_size.0 == 0 {
            Covers::Nothing
        } else {
            let (file, memory) = (
                placed.offset.0..placed.offset.0 + placed.file_size.0,
                placed.address.0..placed.address.0 + placed.memory_size.0,
            );
            // A loaded section is in the segment where its bytes are, or its zeros are in memory.
            let covered: Vec<usize> = (0..headers.len())
                .filter(|&at| {
                    let h = &headers[at];
                    let loaded = SectionFlags(h.flags.0).contains(SectionFlags::ALLOC);
                    let zeros = SectionKind::of(h.kind) == SectionKind::NoBits;
                    loaded && if zeros { memory.contains(&h.address.0) } else { file.contains(&h.offset.0) }
                })
                .collect();
            let (first, last) = (covered.first().copied().unwrap_or(0), covered.last().map_or(0, |&l| l + 1));
            if last - first != covered.len() {
                return Err(Malformed::Segment(k));
            }
            Covers::Sections { headers: placed.offset.0 < header_size, sections: first..last }
        };
        Ok(Self { kind, flags: SegmentFlags(placed.flags), align: placed.align.0, covers })
    }
}
