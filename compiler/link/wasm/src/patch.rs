//! The objects' code and data as the program holds them: each relocation's field rewritten in
//! place, as the object padded it, to what its symbol leads to in the program.

use vun_bytes::Out;
use vun_wasm::{Reloc, RelocKind};

use crate::layout::Value;
use crate::program::{Program, Table};
use crate::unlinked::Unlinked;

impl Program<'_> {
    /// Each object's code and data payloads, relocated, and the table their relocations fill.
    ///
    /// # Errors
    /// Where a relocation names a symbol of a kind it cannot.
    pub(crate) fn relocated(&self) -> Result<(Vec<(Out, Out)>, Table), Unlinked> {
        let objects = self.symbols.objects;
        let mut table = Table::default();
        let relocated = objects.iter().enumerate().map(|(o, object)| {
            // Only what the program keeps: a function left out takes no slot in the table.
            let mut code = object.code.payload.clone();
            for reloc in
                object.code.relocs.iter().filter(|r| self.layout.keeps_func(o, &object.code.bodies, r.offset))
            {
                self.rewrite(o, &mut code, reloc, &mut table)?;
            }
            let mut data = object.data.payload.clone();
            for reloc in object
                .data
                .relocs
                .iter()
                .filter(|r| self.layout.keeps_seg(o, &object.data.segments, r.offset))
            {
                self.rewrite(o, &mut data, reloc, &mut table)?;
            }
            Ok((code, data))
        });
        let relocated = relocated.collect::<Result<_, _>>()?;
        Ok((relocated, table))
    }

    fn rewrite(&self, o: usize, bytes: &mut Out, reloc: &Reloc, table: &mut Table) -> Result<(), Unlinked> {
        let at = reloc.offset;
        if reloc.kind == RelocKind::TypeIndexLeb {
            bytes.rewrite_uleb_padded(at, self.layout.ty(o, reloc.index));
            return Ok(());
        }
        let value = self.layout.value(self.symbols.resolved[o][reloc.index as usize]);
        let wrong = Unlinked::Relocation { input: o, symbol: reloc.index };
        match (reloc.kind, value) {
            (RelocKind::FunctionIndexLeb, Value::Func { index, .. })
            | (RelocKind::GlobalIndexLeb, Value::Global(index))
            | (RelocKind::TableNumberLeb, Value::Table(index)) => bytes.rewrite_uleb_padded(at, index),
            (RelocKind::TableIndexSleb | RelocKind::TableIndexI32, Value::Func { index, missing }) => {
                let slot = if missing { 0 } else { table.slot(index) };
                if reloc.kind == RelocKind::TableIndexI32 {
                    bytes.rewrite_u32(at, slot);
                } else {
                    bytes.rewrite_sleb_padded(at, i32::try_from(slot).map_err(|_| wrong)?);
                }
            }
            (
                RelocKind::MemoryAddrLeb | RelocKind::MemoryAddrSleb | RelocKind::MemoryAddrI32,
                Value::Addr(address),
            ) => {
                let address = address.wrapping_add_signed(reloc.addend);
                match reloc.kind {
                    RelocKind::MemoryAddrLeb => bytes.rewrite_uleb_padded(at, address),
                    RelocKind::MemoryAddrI32 => bytes.rewrite_u32(at, address),
                    #[expect(clippy::cast_possible_wrap, reason = "an address, as `i32.const` holds it")]
                    _ => bytes.rewrite_sleb_padded(at, address as i32),
                }
            }
            _ => return Err(wrong),
        }
        Ok(())
    }
}
