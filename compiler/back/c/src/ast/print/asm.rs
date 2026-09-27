//! Blocks of inline assembly as GNU C writes them: a braced statement declaring each binding's
//! register variable, the `__asm__ volatile` statement over them, then what the block's outputs
//! become.

use std::fmt::{self, Write};

use super::Printer;
use crate::ast::{Asm, Flow};

/// A binding's constraint: an output, one read first, or an input, in a general register.
const OUT: &str = "=r";
const IN_OUT: &str = "+r";
const IN: &str = "r";

impl Printer<'_, '_, '_> {
    pub(super) fn asm(&mut self, asm: &Asm) -> fmt::Result {
        self.f.write_str("{\n")?;
        self.depth += 1;
        for binding in &asm.bindings {
            self.indent()?;
            self.f.write_str("register ")?;
            self.decl(&binding.decl)?;
            self.f.write_str(" __asm__(")?;
            self.string(&binding.reg)?;
            self.f.write_char(')')?;
            if let Flow::In(value) | Flow::InOut(value) = &binding.flow {
                self.f.write_str(" = ")?;
                self.expr(value)?;
            }
            self.f.write_str(";\n")?;
        }
        self.indent()?;
        self.f.write_str("__asm__ volatile(")?;
        self.string(&asm.lines.join("\n\t"))?;
        let outputs = asm.bindings.iter().filter_map(|b| match b.flow {
            Flow::Out => Some((OUT, b.decl.name)),
            Flow::InOut(_) => Some((IN_OUT, b.decl.name)),
            Flow::In(_) => None,
        });
        let inputs = asm.bindings.iter().filter_map(|b| match b.flow {
            Flow::In(_) => Some((IN, b.decl.name)),
            Flow::Out | Flow::InOut(_) => None,
        });
        for operands in [outputs.collect::<Vec<_>>(), inputs.collect()] {
            self.f.write_str(" : ")?;
            for (k, (constraint, name)) in operands.into_iter().enumerate() {
                if k > 0 {
                    self.f.write_str(", ")?;
                }
                self.string(constraint)?;
                self.f.write_char('(')?;
                self.ident(name)?;
                self.f.write_char(')')?;
            }
        }
        self.f.write_str(" : ")?;
        for (k, clobber) in asm.clobbers.iter().enumerate() {
            if k > 0 {
                self.f.write_str(", ")?;
            }
            self.string(clobber)?;
        }
        self.f.write_str(");\n")?;
        self.stmts(&asm.after)?;
        self.depth -= 1;
        self.indent()?;
        self.f.write_char('}')
    }

    /// `text` as a C string literal.
    fn string(&mut self, text: &str) -> fmt::Result {
        self.f.write_char('"')?;
        for c in text.chars() {
            match c {
                '\\' | '"' => write!(self.f, "\\{c}")?,
                '\n' => self.f.write_str("\\n")?,
                '\t' => self.f.write_str("\\t")?,
                _ => self.f.write_char(c)?,
            }
        }
        self.f.write_char('"')
    }
}
