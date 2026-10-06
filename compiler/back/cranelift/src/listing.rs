//! The code as text for people: each function's Cranelift IR and the machine code it compiled to.

use std::fmt;

use vun_backend::Lines;
use vun_index::Idx;
use vun_lir::{FuncId, Unit};

use crate::compile::Listed;

pub(crate) struct Listing<'a> {
    pub(crate) unit: &'a Unit,
    pub(crate) functions: &'a [(FuncId, Listed)],
    pub(crate) lines: &'a dyn Lines,
}

impl fmt::Display for Listing<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, Listed { text, origins }) in self.functions {
            let origin = self.unit.functions[*id].origin;
            match self.lines.name(origin) {
                Some(name) => writeln!(f, "; {name}")?,
                None => writeln!(f, "; function {}", id.index())?,
            }
            writeln!(f, "{text}")?;
            for (n, &origin) in origins.iter().enumerate() {
                match self.lines.line(origin) {
                    Some(at) => writeln!(f, "; @{n:04x} {}:{}:{}", at.file.display(), at.line, at.column)?,
                    None => writeln!(f, "; @{n:04x} (no source)")?,
                }
            }
        }
        Ok(())
    }
}
