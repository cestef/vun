//! Views of what the middle and the low end build: each function's MIR, the program a root file
//! exports, and its LIR.

use std::io::{self, Write};
use std::sync::Arc;

use vun_mir::{Body, Concrete, Mir, Unbuilt};
use vun_origin::{OriginId, Resolve};
use vun_span::{FileId, SourceMap};

use crate::Session;

impl Session {
    pub(super) fn view_mir(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        for (f, body) in self.bodies(file) {
            self.write_body(f.expr.origin, body, out)?;
        }
        Ok(())
    }

    pub(super) fn view_program(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let program = self.program(file);
        for &(at, symbol) in &program.exports {
            writeln!(out, "export {symbol}: fn {at}")?;
        }
        for (at, i) in program.instantiations().enumerate() {
            let origin = i.f.literal.expr.origin;
            writeln!(out, "fn {at} at {}", self.sources.locate(self.source_span(origin)))?;
            match self.db.get::<Concrete>(i) {
                Ok(body) => self.write_body(origin, Ok(body), out)?,
                Err(unready) => {
                    let at = self.sources.locate(self.source_span(unready.at()));
                    writeln!(out, "not ready at {at}: {unready}")?;
                    self.write_body(origin, self.db.get::<Mir>(i), out)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn view_lir(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let lowered = self.lir(file);
        let locate = |o| self.sources.locate(self.source_span(o)).at;
        match &*lowered {
            Ok(unit) => {
                write!(out, "{}", unit.dump(&locate))?;
                if let Err(malformed) = unit.verify(self.origins) {
                    writeln!(out, "malformed: {malformed}")?;
                }
            }
            Err(unlowered) => {
                let at = self.sources.locate(self.source_span(unlowered.at));
                writeln!(out, "not lowered at {at}: {unlowered}")?;
            }
        }
        Ok(())
    }

    /// Writes a function's body with the origin of each line, or where lowering it stopped.
    fn write_body(
        &self,
        origin: OriginId,
        body: Result<Arc<Body>, Unbuilt>,
        out: &mut dyn Write,
    ) -> io::Result<()> {
        let locate = |o| self.sources.locate(self.source_span(o)).at;
        match body {
            Ok(body) => writeln!(out, "{}", body.dump(&locate)),
            Err(unbuilt) => {
                let (at, form) = (locate(origin), locate(unbuilt.at));
                let (line, col) = (form.line, form.col);
                writeln!(
                    out,
                    "fn @{}:{}: not lowered yet, at @{line}:{col}, by {}",
                    at.line, at.col, unbuilt.by
                )
            }
        }
    }
}
