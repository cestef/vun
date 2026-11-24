//! The code of a PE file: its executable sections, and the names it exports, its entry, and the
//! slots it imports through, which its code calls.

use vun_pe::{Image, Machine};

use crate::Isa;
use crate::code::{Code, Error, Part, Symbol};

impl Code {
    /// The code of `image`.
    ///
    /// # Errors
    /// Where it is for a machine whose instructions are not read.
    pub fn of_pe(image: &Image) -> Result<Self, Error> {
        let isa = match image.machine {
            Machine::X86 => Isa::X86,
            Machine::X86_64 => Isa::X86_64,
            Machine::Arm64 => Isa::Arm64,
            other => return Err(Error::PeMachine(other)),
        };
        let parts = image
            .sections
            .iter()
            .filter(|s| s.executable())
            .map(|s| Part::loaded(&s.name, s.address, s.data.clone()))
            .collect::<Result<Vec<Part>, _>>()?;
        let inside = |address: u64| {
            parts.iter().position(|p| (p.address..p.address + p.bytes.len() as u64).contains(&address))
        };
        let mut symbols = Vec::new();
        if image.entry != 0 {
            symbols.push(Symbol {
                name: b"entry".to_vec(),
                address: image.entry,
                size: 0,
                part: inside(image.entry),
            });
        }
        for export in image.exports.iter().filter(|e| e.forwarder.is_none()) {
            let name =
                export.name.clone().unwrap_or_else(|| format!("ordinal_{}", export.ordinal).into_bytes());
            symbols.push(Symbol { name, address: export.address, size: 0, part: inside(export.address) });
        }
        let word = if image.wide { 8 } else { 4 };
        for import in &image.imports {
            let name = match (&import.name, import.ordinal) {
                (Some(name), _) => format!("__imp_{}", String::from_utf8_lossy(name)),
                (None, Some(ordinal)) => {
                    format!("__imp_{}!{ordinal}", String::from_utf8_lossy(&import.library))
                }
                (None, None) => continue,
            };
            symbols.push(Symbol { name: name.into_bytes(), address: import.slot, size: word, part: None });
        }
        Ok(Self { isa, parts, symbols, linked: true })
    }
}
