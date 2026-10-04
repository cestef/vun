//! What a loader puts in memory: the bytes each loadable segment holds, at the address it is
//! loaded from, which a program placed in flash differs from where it runs in (`.data`).

use vun_bytes::In;

use crate::headers::{FileHeader, ProgramHeader};
use crate::{Class, File, LITTLE, Malformed, SegmentKind};

/// The bytes of a loadable segment, and where in physical memory a loader puts them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Load<'a> {
    pub physical: u64,
    pub bytes: &'a [u8],
}

impl File {
    /// The loadable segments of the file `bytes` hold, in the order of its program headers, those
    /// holding no bytes left out.
    ///
    /// # Errors
    /// Where `bytes` are not a little-endian ELF file, or a header or a segment runs past their end.
    pub fn loads(bytes: &[u8]) -> Result<Vec<Load<'_>>, Malformed> {
        let ident: [u8; 16] = In::at(bytes, 0).array()?;
        let class =
            Class::of_ident(ident[crate::read::CLASS]).ok_or(Malformed::Class(ident[crate::read::CLASS]))?;
        if ident[crate::read::DATA] != LITTLE {
            return Err(Malformed::BigEndian);
        }
        let header = FileHeader::take(class, &mut In::at(bytes, 0))?;
        let mut input = In::at(bytes, header.program_headers.0);
        let mut loads = Vec::new();
        for _ in 0..header.program_header_count {
            let placed = ProgramHeader::take(class, &mut input)?;
            if SegmentKind::of(placed.kind) == SegmentKind::Load && placed.file_size.0 > 0 {
                let held = In::at(bytes, placed.offset.0).take(placed.file_size.0)?;
                loads.push(Load { physical: placed.physical.0, bytes: held });
            }
        }
        Ok(loads)
    }
}
