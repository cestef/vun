//! The LIR text format, which dumps print and external backends read: imports, data and
//! functions by id, each function's slots and blocks with their parameters, one instruction per
//! line, and in dumps the source position of each line's origin.

mod inst;

use std::fmt;

use vun_origin::OriginId;
use vun_span::LineCol;

use crate::func::{AbiType, Alias, Conv, Ext, Function, Linkage, Record, Signature};
use crate::unit::{ImportKind, Unit};

impl fmt::Display for Conv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Vun => "vun",
            Self::C => "c",
            Self::Naked => "naked",
        })
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{} align {}", self.size, self.align)?;
        for (i, (offset, scalar)) in self.fields.iter().enumerate() {
            write!(f, "{}{offset} {scalar}", if i == 0 { ": " } else { ", " })?;
        }
        f.write_str("}")
    }
}

impl fmt::Display for AbiType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(s) => s.fmt(f),
            Self::Int(s, Ext::Zero) => write!(f, "{s} zext"),
            Self::Int(s, Ext::Sign) => write!(f, "{s} sext"),
            Self::Address(Alias::Any) => f.write_str("ptr"),
            Self::Address(Alias::Exclusive) => f.write_str("ptr noalias"),
            Self::Record(r) => r.fmt(f),
            Self::Memory { size, align } => write!(f, "[{size} align {align}]"),
        }
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.conv)?;
        for (i, p) in self.params.iter().enumerate() {
            write!(f, "{}{p}", if i > 0 { ", " } else { "" })?;
        }
        f.write_str(")")?;
        match &self.result {
            Some(result) => write!(f, " -> {result}"),
            None => Ok(()),
        }
    }
}

impl fmt::Display for Linkage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal => Ok(()),
            Self::Export(symbol) => write!(f, " export {:?}", symbol.as_str()),
        }
    }
}

/// A unit as text; with a locator, each line ends with the source position of its origin.
pub struct Text<'u> {
    unit: &'u Unit,
    locate: Option<&'u dyn Fn(OriginId) -> LineCol>,
}

impl Unit {
    /// This unit's text with the origin of each line, found by `locate`, as `vun dump lir`
    /// prints it.
    #[must_use]
    pub fn dump<'u>(&'u self, locate: &'u dyn Fn(OriginId) -> LineCol) -> Text<'u> {
        Text { unit: self, locate: Some(locate) }
    }
}

impl Text<'_> {
    /// ` stack.max(n)` for a function or import bound to `n` bytes of stack, else nothing.
    fn stack(f: &mut fmt::Formatter<'_>, stack_max: Option<u64>) -> fmt::Result {
        match stack_max {
            Some(bytes) => write!(f, " stack.max({bytes})"),
            None => Ok(()),
        }
    }

    /// Ends a line with the position of `origin`, when locating.
    fn end(&self, f: &mut fmt::Formatter<'_>, origin: OriginId) -> fmt::Result {
        match self.locate {
            Some(locate) => {
                let at = locate(origin);
                writeln!(f, " @{}:{}", at.line, at.col)
            }
            None => writeln!(f),
        }
    }

    fn function(&self, f: &mut fmt::Formatter<'_>, func: &Function) -> fmt::Result {
        for (slot, data) in func.slots.iter() {
            write!(f, "  slot {slot}: {} align {}", data.size, data.align)?;
            self.end(f, data.origin)?;
        }
        for (block, data) in func.blocks.iter() {
            write!(f, "{block}")?;
            if !data.params.is_empty() {
                f.write_str("(")?;
                for (i, &p) in data.params.iter().enumerate() {
                    write!(f, "{}{p}: {}", if i > 0 { ", " } else { "" }, func.ty(p))?;
                }
                f.write_str(")")?;
            }
            f.write_str(":")?;
            self.end(f, data.origin)?;
            for &inst in &data.insts {
                let inst = &func.insts[inst];
                f.write_str("  ")?;
                if let Some(result) = inst.result {
                    write!(f, "{result} = ")?;
                }
                write!(f, "{}", inst.kind)?;
                self.end(f, inst.origin)?;
            }
            write!(f, "  {}", data.term)?;
            self.end(f, data.origin)?;
        }
        Ok(())
    }
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = self.unit;
        writeln!(f, "unit ptr {}", unit.pointer_bits)?;
        for (id, import) in unit.imports.iter() {
            write!(f, "import {id} {:?}", import.symbol.as_str())?;
            match &import.kind {
                ImportKind::Func(signature) => write!(f, ": {signature}")?,
                ImportKind::Data => f.write_str(": data")?,
            }
            Self::stack(f, import.stack_max)?;
            self.end(f, import.origin)?;
        }
        for (id, data) in unit.data.iter() {
            let writable = if data.writable { " writable" } else { "" };
            write!(f, "data {id}{} align {}{writable} [", data.linkage, data.align)?;
            for (i, byte) in data.bytes.iter().enumerate() {
                write!(f, "{}{byte:02x}", if i > 0 { " " } else { "" })?;
            }
            f.write_str("]")?;
            for reloc in &data.relocs {
                write!(f, " at {}: {} + {}", reloc.offset, reloc.target, reloc.addend)?;
            }
            self.end(f, data.origin)?;
        }
        for (id, func) in unit.functions.iter() {
            write!(f, "fn {id}{}: {}", func.linkage, func.signature)?;
            Self::stack(f, func.stack_max)?;
            write!(f, " {{")?;
            self.end(f, func.origin)?;
            self.function(f, func)?;
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Text { unit: self, locate: None }.fmt(f)
    }
}
