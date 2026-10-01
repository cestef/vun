//! The tables a PE file names its exports and imports in.

use std::collections::HashMap;

use vun_bytes::In;

use crate::headers::Malformed;
use crate::{Export, Image, Import};

/// Bytes of the export directory, and of an import descriptor.
const EXPORT_DIRECTORY: u64 = 40;
const IMPORT_DESCRIPTOR: u64 = 20;
/// The bit of an import thunk that makes the rest an ordinal, in a 64-bit image and a 32-bit one.
const ORDINAL_64: u64 = 1 << 63;
const ORDINAL_32: u64 = 1 << 31;

impl Image {
    /// The exports of the directory at `(offset, size)`, none where it has no offset.
    pub(crate) fn exports_at(&self, (rva, size): (u64, u64)) -> Result<Vec<Export>, Malformed> {
        if rva == 0 {
            return Ok(Vec::new());
        }
        let mut dir = In::at(self.at_rva(rva, EXPORT_DIRECTORY)?, 12);
        let _library_name = dir.u32()?;
        let ordinal_base = dir.u32()?;
        let functions = u64::from(dir.u32()?);
        let names = u64::from(dir.u32()?);
        let (functions_at, names_at, ordinals_at) =
            (u64::from(dir.u32()?), u64::from(dir.u32()?), u64::from(dir.u32()?));
        let mut by_ordinal: HashMap<u16, Vec<u8>> = HashMap::new();
        let mut name_rvas = In::at(self.at_rva(names_at, 4 * names)?, 0);
        let mut ordinals = In::at(self.at_rva(ordinals_at, 2 * names)?, 0);
        for _ in 0..names {
            let name = self.string_at_rva(u64::from(name_rvas.u32()?))?;
            by_ordinal.insert(ordinals.u16()?, name.to_vec());
        }
        let mut addresses = In::at(self.at_rva(functions_at, 4 * functions)?, 0);
        let mut exports = Vec::new();
        for index in 0..functions {
            let target = u64::from(addresses.u32()?);
            if target == 0 {
                continue;
            }
            // An address inside the directory names the function that stands for this one.
            let forwarder =
                (rva..rva + size).contains(&target).then(|| self.string_at_rva(target)).transpose()?;
            exports.push(Export {
                name: u16::try_from(index).ok().and_then(|i| by_ordinal.get(&i).cloned()),
                ordinal: ordinal_base + u32::try_from(index).map_err(|_| Malformed::Rva(rva))?,
                address: self.base + target,
                forwarder: forwarder.map(<[u8]>::to_vec),
            });
        }
        Ok(exports)
    }

    /// The imports of the descriptors at `(offset, size)`, none where it has no offset.
    pub(crate) fn imports_at(&self, (rva, _size): (u64, u64)) -> Result<Vec<Import>, Malformed> {
        if rva == 0 {
            return Ok(Vec::new());
        }
        let (word, ordinal_bit) = if self.wide { (8, ORDINAL_64) } else { (4, ORDINAL_32) };
        let mut imports = Vec::new();
        for k in 0.. {
            let mut descriptor = In::at(self.at_rva(rva + k * IMPORT_DESCRIPTOR, IMPORT_DESCRIPTOR)?, 0);
            let lookup = u64::from(descriptor.u32()?);
            descriptor.take(8)?;
            let library_rva = u64::from(descriptor.u32()?);
            let slots = u64::from(descriptor.u32()?);
            if library_rva == 0 {
                break;
            }
            let library = self.string_at_rva(library_rva)?.to_vec();
            // The names are where the lookup table says, or, where there is none, in the slots.
            let table = if lookup == 0 { slots } else { lookup };
            for i in 0.. {
                let mut thunk = In::at(self.at_rva(table + i * word, word)?, 0);
                let value = if self.wide { thunk.u64()? } else { u64::from(thunk.u32()?) };
                if value == 0 {
                    break;
                }
                let slot = self.base + slots + i * word;
                imports.push(if value & ordinal_bit != 0 {
                    Import {
                        library: library.clone(),
                        name: None,
                        ordinal: u16::try_from(value & 0xffff).ok(),
                        slot,
                    }
                } else {
                    // A hint, then the name.
                    let name = self.string_at_rva((value & !ordinal_bit) + 2)?.to_vec();
                    Import { library: library.clone(), name: Some(name), ordinal: None, slot }
                });
            }
        }
        Ok(imports)
    }
}
