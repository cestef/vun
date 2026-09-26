//! The listing of a unit's machine code (`vun debug code --backend native`): each function under
//! its name, its instructions with their offsets and bytes, and before the code of each LIR
//! instruction the line of source it comes from, where that line changes.

use std::fmt;

use vun_backend::Lines;
use vun_index::IndexVec;
use vun_lir::{FuncId, Unit};

use crate::Code;

pub struct Listing<'a> {
    pub unit: &'a Unit,
    pub code: &'a IndexVec<FuncId, Code>,
    pub lines: &'a dyn Lines,
}

impl fmt::Display for Listing<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, compiled) in self.code.iter() {
            let function = &self.unit.functions[id];
            writeln!(f, "{id} {}, frame {} bytes:", function.linkage, compiled.frame)?;
            let mut marks = compiled.marks.iter().peekable();
            let mut shown = None;
            for line in &compiled.code.listing {
                if let vun_asm::Line::Inst { at, .. } = line {
                    while let Some(&&(mark, origin)) = marks.peek()
                        && mark <= *at
                    {
                        marks.next();
                        if let Some(source) = self.lines.line(origin)
                            && shown.as_ref() != Some(&source)
                        {
                            writeln!(f, "  ; {}:{}", source.file.display(), source.line)?;
                            shown = Some(source);
                        }
                    }
                }
                writeln!(f, "{}", line.show(&compiled.code.bytes))?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
