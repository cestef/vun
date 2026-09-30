//! The entries of an object's sections: bodies and segments, types, imports, exports, COMDAT
//! groups and relocations.

use crate::format::{ComdatKind, External, FUNC_TYPE, FUNCREF, FuncType, Import, MUTABLE, RelocKind, op};
use crate::input::{Coded, Input, Malformed};
use crate::object::{Code, Comdat, Data, ObjectImport, Reloc, Segment};

/// A data segment active in memory 0 at a constant offset, the only kind objects hold.
const ACTIVE: u32 = 0;

impl Code {
    pub(super) fn read(payload: &[u8]) -> Result<Self, Malformed> {
        let mut input = Input::new(payload);
        let bodies = input.vector(|input| {
            let size = input.uleb()? as usize;
            let start = input.at;
            input.take(size)?;
            Ok(start..start + size)
        })?;
        Ok(Self { payload: vun_bytes::Out(payload.to_vec()), bodies, relocs: Vec::new() })
    }
}

impl Data {
    pub(super) fn read(payload: &[u8]) -> Result<Self, Malformed> {
        let mut input = Input::new(payload);
        let segments = input.vector(|input| {
            let flags = input.uleb()?;
            if flags != ACTIVE {
                return Err(Malformed::Unsupported { what: Coded::Segment, code: flags });
            }
            input.expect(op::I32_CONST)?;
            input.sleb()?;
            input.expect(op::END)?;
            let size = input.uleb()? as usize;
            let start = input.at;
            input.take(size)?;
            Ok(Segment { bytes: start..start + size, name: String::new(), align: 1 })
        })?;
        Ok(Self { payload: vun_bytes::Out(payload.to_vec()), segments, relocs: Vec::new() })
    }
}

impl Input<'_> {
    pub(super) fn func_type(&mut self) -> Result<FuncType, Malformed> {
        self.expect(FUNC_TYPE)?;
        Ok(FuncType { params: self.vector(Self::val_type)?, results: self.vector(Self::val_type)? })
    }

    /// The byte `code`, as a constant expression or a type spells it.
    pub(super) fn expect(&mut self, code: u8) -> Result<(), Malformed> {
        let at = self.at;
        let found = self.u8()?;
        if found == code {
            Ok(())
        } else {
            Err(Malformed::Code { what: Coded::Constant, code: u32::from(found), at })
        }
    }

    pub(super) fn import(&mut self) -> Result<ObjectImport, Malformed> {
        let module = self.name()?.to_owned();
        let name = self.name()?.to_owned();
        let import = match self.coded(Coded::Import, External::of)? {
            External::Func => Import::Func(self.uleb()?),
            External::Table => {
                self.expect(FUNCREF)?;
                Import::Table(self.limits()?)
            }
            External::Memory => Import::Memory(self.limits()?),
            External::Global => Import::Global { ty: self.val_type()?, mutable: self.u8()? == MUTABLE },
        };
        Ok(ObjectImport { module, name, import })
    }

    /// A function's export, or none for another kind's.
    pub(super) fn export(&mut self) -> Result<Option<(String, u32)>, Malformed> {
        let name = self.name()?.to_owned();
        let kind = self.coded(Coded::Import, External::of)?;
        let index = self.uleb()?;
        Ok((kind == External::Func).then_some((name, index)))
    }

    pub(super) fn comdat(&mut self) -> Result<Comdat, Malformed> {
        let name = self.name()?.to_owned();
        // Flags, which no version defines yet.
        self.uleb()?;
        let mut comdat = Comdat { name, funcs: Vec::new(), segments: Vec::new() };
        for _ in 0..self.uleb()? {
            let kind = self.coded(Coded::Comdat, ComdatKind::of)?;
            let index = self.uleb()?;
            match kind {
                ComdatKind::Function => comdat.funcs.push(index),
                ComdatKind::Data => comdat.segments.push(index),
                other => return Err(Malformed::Unsupported { what: Coded::Comdat, code: other as u32 }),
            }
        }
        Ok(comdat)
    }

    pub(super) fn reloc(&mut self) -> Result<Reloc, Malformed> {
        let code = self.u8()?;
        let kind = RelocKind::of(code)
            .ok_or(Malformed::Unsupported { what: Coded::Reloc, code: u32::from(code) })?;
        let offset = self.uleb()? as usize;
        let index = self.uleb()?;
        let addend = if kind.has_addend() { self.sleb()? } else { 0 };
        Ok(Reloc { kind, offset, index, addend })
    }
}
