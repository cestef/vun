//! Statements, indented four spaces a level; `#line` and labels at the margin's level.

use std::fmt::{self, Write};

use super::Printer;
use crate::ast::Stmt;

impl Printer<'_, '_, '_> {
    pub(super) fn stmts(&mut self, stmts: &[Stmt]) -> fmt::Result {
        for (i, stmt) in stmts.iter().enumerate() {
            self.stmt(stmt)?;
            if let Stmt::Label(_) = stmt {
                // A label must label a statement.
                if matches!(stmts.get(i + 1), None | Some(Stmt::Local(_))) {
                    self.f.write_char(';')?;
                }
                self.f.write_char('\n')?;
            }
        }
        Ok(())
    }

    fn indent(&mut self) -> fmt::Result {
        for _ in 0..self.depth {
            self.f.write_str("    ")?;
        }
        Ok(())
    }

    /// The statements of a switch's arm, one level in.
    fn arm(&mut self, stmts: &[Stmt]) -> fmt::Result {
        self.depth += 1;
        self.stmts(stmts)?;
        self.depth -= 1;
        Ok(())
    }

    /// Statements one level in, then the indentation of the brace closing them.
    fn block(&mut self, stmts: &[Stmt]) -> fmt::Result {
        self.depth += 1;
        self.stmts(stmts)?;
        self.depth -= 1;
        self.indent()
    }

    fn stmt(&mut self, stmt: &Stmt) -> fmt::Result {
        match stmt {
            Stmt::Line(line) => {
                write!(self.f, "#line {} \"", line.line)?;
                // The path's bytes as they are: printable ones plainly, the others in octal, so any
                // path, whatever its encoding, names its file; `?` escaped, as C99 reads `??/` as
                // a trigraph.
                for &byte in line.file.as_os_str().as_encoded_bytes() {
                    match byte {
                        b'\\' | b'"' | b'?' => write!(self.f, "\\{}", char::from(byte))?,
                        b' '..=b'~' => self.f.write_char(char::from(byte))?,
                        _ => write!(self.f, "\\{byte:03o}")?,
                    }
                }
                return self.f.write_str("\"\n");
            }
            Stmt::Label(label) => {
                self.f.write_str("\n")?;
                self.depth -= 1;
                self.indent()?;
                self.depth += 1;
                self.ident(*label)?;
                return self.f.write_char(':');
            }
            _ => {}
        }
        self.indent()?;
        match stmt {
            Stmt::Local(decl) => {
                self.decl(decl)?;
                self.f.write_char(';')?;
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
                self.f.write_char(';')?;
            }
            Stmt::Assign(to, e) => {
                self.expr(to)?;
                self.f.write_str(" = ")?;
                self.expr(e)?;
                self.f.write_char(';')?;
            }
            Stmt::Goto(label) => {
                self.f.write_str("goto ")?;
                self.ident(*label)?;
                self.f.write_char(';')?;
            }
            Stmt::If(cond, then, otherwise) => {
                self.f.write_str("if (")?;
                self.expr(cond)?;
                if let ([Stmt::Goto(label)], []) = (then.as_slice(), otherwise.as_slice()) {
                    self.f.write_str(") goto ")?;
                    self.ident(*label)?;
                    self.f.write_char(';')?;
                } else {
                    self.f.write_str(") {\n")?;
                    self.block(then)?;
                    if !otherwise.is_empty() {
                        self.f.write_str("} else {\n")?;
                        self.block(otherwise)?;
                    }
                    self.f.write_char('}')?;
                }
            }
            Stmt::Switch(value, cases, default) => {
                self.f.write_str("switch (")?;
                self.expr(value)?;
                self.f.write_str(") {\n")?;
                for (case, body) in cases {
                    self.indent()?;
                    self.f.write_str("case ")?;
                    self.expr(case)?;
                    self.f.write_str(":\n")?;
                    self.arm(body)?;
                }
                self.indent()?;
                self.f.write_str("default:\n")?;
                self.arm(default)?;
                self.indent()?;
                self.f.write_char('}')?;
            }
            Stmt::Return(value) => {
                self.f.write_str("return")?;
                if let Some(value) = value {
                    self.f.write_char(' ')?;
                    self.expr(value)?;
                }
                self.f.write_char(';')?;
            }
            Stmt::Halt => self.f.write_str("for (;;) {}")?,
            Stmt::Line(_) | Stmt::Label(_) => unreachable!("printed above"),
        }
        self.f.write_char('\n')
    }
}
