//! The listing of a module (`vun debug code --backend wasm`): each function under its name and
//! type, its instructions in the text format, indented by block, and before the code of each LIR
//! instruction the line of source it comes from, where that line changes.

use std::fmt;

use vun_backend::Lines;

use crate::module::op::Op;
use crate::module::{Def, Kind, Module, Text};

pub struct Listing<'a> {
    pub module: &'a Module,
    pub lines: &'a dyn Lines,
}

impl fmt::Display for Listing<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (_, sym) in self.module.symbols.iter() {
            let Kind::Func { ty, def: Def::Here(at) } = sym.kind else { continue };
            let func = &self.module.funcs[at];
            let ty = &self.module.types[ty];
            let list = |tys: &[crate::module::ValType]| {
                tys.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")
            };
            writeln!(
                f,
                "func ${} (param {}) (result {}), {} locals:",
                sym.name,
                list(&ty.params),
                list(&ty.results),
                func.locals.len()
            )?;
            let mut marks = func.marks.iter().peekable();
            let (mut shown, mut depth) = (None, 1);
            for (i, op) in func.body.iter().enumerate() {
                while let Some(&&(mark, origin)) = marks.peek()
                    && mark <= i
                {
                    marks.next();
                    if let Some(source) = self.lines.line(origin)
                        && shown.as_ref() != Some(&source)
                    {
                        writeln!(f, "  ; {}:{}", source.file.display(), source.line)?;
                        shown = Some(source);
                    }
                }
                if matches!(op, Op::End | Op::Else) {
                    depth -= 1;
                }
                writeln!(f, "{:width$}{}", "", Text { module: self.module, op }, width = 2 * depth)?;
                if matches!(op, Op::Block | Op::Loop | Op::If | Op::Else) {
                    depth += 1;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
