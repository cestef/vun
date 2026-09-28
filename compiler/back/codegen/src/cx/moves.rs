//! Running the plan's moves: between registers, homes and the framework's register for cycles
//! of moves; a home copied to another through the carrier, a value recomputed where it goes.

use vun_regalloc::{Loc, Move, Source};

use super::Cx;
use crate::Arch;

impl<A: Arch> Cx<'_, A> {
    /// Runs `moves`, in order.
    pub(super) fn run(&mut self, moves: &[Move]) {
        for &m in moves {
            let from = self.source(m.from);
            match m.to {
                Loc::Reg(p) => {
                    let dst = self.reg_of(p);
                    self.write(dst, from);
                }
                Loc::Temp => self.write(A::TEMP, from),
                // A value wider than a register goes from home to home a register at a time.
                Loc::Home(v) if self.home_bytes(v) > A::WORD => {
                    let Written::Home(at) = from else {
                        unreachable!("a value wider than a register is in no register")
                    };
                    let home = self.home(v);
                    for word in (0..self.home_bytes(v)).step_by(usize::try_from(A::WORD).expect("a word")) {
                        let word = i32::try_from(word).expect("a home's bytes");
                        A::load(&mut self.asm, A::CARRIER, at + word);
                        A::store(&mut self.asm, home + word, A::CARRIER);
                    }
                }
                Loc::Home(v) => {
                    let src = match from {
                        Written::Reg(r) => r,
                        from => {
                            self.write(A::CARRIER, from);
                            A::CARRIER
                        }
                    };
                    let home = self.home(v);
                    A::store(&mut self.asm, home, src);
                }
            }
        }
    }

    fn source(&mut self, from: Source) -> Written<A::Reg> {
        match from {
            Source::At(Loc::Reg(p)) => Written::Reg(self.reg_of(p)),
            Source::At(Loc::Temp) => Written::Reg(A::TEMP),
            Source::At(Loc::Home(v)) => Written::Home(self.home(v)),
            Source::Remade(v) => Written::Remade(self.made(v).expect("a value the plan makes")),
        }
    }

    /// Writes into `dst` what `from` reads.
    fn write(&mut self, dst: A::Reg, from: Written<A::Reg>) {
        match from {
            Written::Reg(r) => A::copy(&mut self.asm, dst, r),
            Written::Home(at) => A::load(&mut self.asm, dst, at),
            Written::Remade(what) => A::recompute(&mut self.asm, dst, what),
        }
    }
}

/// Where a move reads, as the machine reaches it.
#[derive(Clone, Copy)]
enum Written<R> {
    Reg(R),
    Home(i32),
    Remade(crate::Recompute),
}
