//! Blocks of inline assembly as GNU C writes them: a braced statement declaring each binding's
//! register variable, the `__asm__ volatile` statement over them, then what the block's outputs
//! become; a block binding and clobbering nothing is the plain statement alone.

use std::fmt::{self, Write};

use super::Printer;
use crate::ast::{Asm, Flow};

impl Printer<'_, '_, '_> {
    pub(super) fn asm(&mut self, asm: &Asm) -> fmt::Result {
        // A block with no bindings and no clobbers is plain `__asm__`, all a naked function holds.
        if asm.bindings.is_empty() && asm.clobbers.is_empty() && asm.after.is_empty() {
            return self.statement(asm);
        }
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
        self.statement(asm)?;
        self.f.write_char('\n')?;
        self.stmts(&asm.after)?;
        self.depth -= 1;
        self.indent()?;
        self.f.write_char('}')
    }

    /// The `__asm__ volatile` statement, with its operands and clobbers where it has any.
    fn statement(&mut self, asm: &Asm) -> fmt::Result {
        self.f.write_str("__asm__ volatile(")?;
        self.string(&asm.lines.join("\n\t"))?;
        if asm.bindings.is_empty() && asm.clobbers.is_empty() {
            return self.f.write_str(");");
        }
        let (outputs, inputs): (Vec<_>, Vec<_>) =
            asm.bindings.iter().partition(|b| !matches!(b.flow, Flow::In(_)));
        for operands in [outputs, inputs] {
            self.f.write_str(" : ")?;
            for (k, binding) in operands.into_iter().enumerate() {
                if k > 0 {
                    self.f.write_str(", ")?;
                }
                // An output is `=`, one read first `+`, then the class.
                let mode = match binding.flow {
                    Flow::Out => "=",
                    Flow::InOut(_) => "+",
                    Flow::In(_) => "",
                };
                write!(self.f, "\"{mode}{}\"(", binding.class.0)?;
                self.ident(binding.decl.name)?;
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
        self.f.write_str(");")
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
