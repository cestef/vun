//! Where values are, as instruction compilers ask: operands in the registers the plan holds
//! them in, results in the ones it gives them, scratch registers it keeps free for the point,
//! and values recomputed where used brought into one.

use vun_asm::imm::{low64, sign_extend};
use vun_lir::Value;
use vun_regalloc::{Bank, Loc, Pos, Remade};

use super::Cx;
use crate::Arch;
use crate::values::{Place, Recompute};

impl<A: Arch> Cx<'_, A> {
    /// How `v` is made where used, if it is: held nowhere.
    #[must_use]
    pub fn recomputed(&self, v: Value) -> Option<Recompute> {
        match self.values[v].made {
            Some((what, Remade::Used)) => Some(what),
            _ => None,
        }
    }

    /// How `v` is made anywhere, if it is, whether where used or held.
    #[must_use]
    pub fn made(&self, v: Value) -> Option<Recompute> {
        self.values[v].made.map(|(what, _)| what)
    }

    /// The constant `v` is, sign-extended from its width, when it is one of at most 64 bits.
    ///
    /// # Panics
    /// Never: the bits are masked to 64.
    #[must_use]
    pub fn imm(&self, v: Value) -> Option<i64> {
        let Some(Recompute::Const(bits)) = self.made(v) else { return None };
        let width = self.f.ty(v).bits(self.unit.pointer_bits);
        (width <= 64).then(|| sign_extend(low64(bits), width))
    }

    /// The address `v` is, when it is known while compiling.
    #[must_use]
    pub fn place(&self, v: Value) -> Option<Place> {
        match self.made(v) {
            Some(Recompute::Addr(place)) => Some(place),
            _ => None,
        }
    }

    /// Where the plan holds `v` as the point compiled now reads it.
    fn loc(&self, v: Value) -> Option<Loc> {
        self.plan().at(v, Pos::read(self.point))
    }

    /// The register holding `v` as the point compiled now reads it, if one does.
    #[must_use]
    pub fn held(&self, v: Value) -> Option<A::Reg> {
        match self.loc(v) {
            Some(Loc::Reg(p)) => Some(self.reg_of(p)),
            _ => None,
        }
    }

    /// The offset of the home holding `v`, when no register does.
    #[must_use]
    pub fn in_home(&self, v: Value) -> Option<i32> {
        match self.loc(v) {
            Some(Loc::Home(_)) => self.values[v].home,
            _ => None,
        }
    }

    /// Makes `v` recomputed as the address `place`: a parameter passed in memory.
    pub fn recompute_as(&mut self, v: Value, place: Place) {
        self.values[v].made = Some((Recompute::Addr(place), Remade::Used));
    }

    /// Makes the home of `v` the memory at `offset` from the frame pointer, which holds it
    /// already: a parameter passed on the stack.
    pub fn home_at(&mut self, v: Value, offset: i32) {
        self.values[v].home = Some(offset);
    }

    /// The offset of `v`'s home, allocated when first needed: a register's bytes, or the value's
    /// where it is wider, as a value of the float bank may be.
    pub fn home(&mut self, v: Value) -> i32 {
        if let Some(at) = self.values[v].home {
            return at;
        }
        let bytes = self.home_bytes(v);
        let at = self.frame.reserve(bytes, bytes);
        self.values[v].home = Some(at);
        at
    }

    /// The register holding `v` for the instruction compiled: the plan's, or for a value
    /// recomputed where used, a scratch register it is written into once per point.
    ///
    /// # Panics
    /// Panics where the plan keeps `v` in memory: the architecture said the instruction takes
    /// it anywhere, and asks for it in a register.
    pub fn reg(&mut self, v: Value) -> A::Reg {
        if let Some(r) = self.held(v) {
            return r;
        }
        if let Some(&(_, r)) = self.remade.iter().find(|&&(w, _)| w == v) {
            return r;
        }
        let what =
            self.recomputed(v).unwrap_or_else(|| panic!("the plan holds {v} in memory at a register use"));
        let r = self.scratch();
        A::recompute(&mut self.asm, r, what);
        self.remade.push((v, r));
        r
    }

    /// Writes `v` into `dst`, which does not then hold it for later uses.
    pub fn reg_to(&mut self, v: Value, dst: A::Reg) {
        match (self.loc(v), self.made(v)) {
            (Some(Loc::Reg(p)), _) => {
                let r = self.reg_of(p);
                A::copy(&mut self.asm, dst, r);
            }
            (_, Some(what)) => A::recompute(&mut self.asm, dst, what),
            _ => {
                let home = self.home(v);
                A::load(&mut self.asm, dst, home);
            }
        }
    }

    /// The register the value `v` the point defines is written to.
    ///
    /// # Panics
    /// Panics where the plan gives the result no register: the architecture said the
    /// instruction writes it elsewhere.
    pub fn def(&mut self, v: Value) -> A::Reg {
        match self.plan().at(v, Pos::write(self.point)) {
            Some(Loc::Reg(p)) => self.reg_of(p),
            other => panic!("the plan writes {v} to {other:?}, not a register"),
        }
    }

    /// A register for `out` holding `v`'s value, to be overwritten by an instruction that reads
    /// and writes it: `v`'s own where the plan gives it to `out`, else a copy.
    pub fn reuse(&mut self, out: Value, v: Value) -> A::Reg {
        let src = self.reg(v);
        let dst = self.def(out);
        A::copy(&mut self.asm, dst, src);
        dst
    }

    /// The register holding `v`, when the plan gives it to `out` as well: the result then
    /// overwrites the operand in place.
    pub fn inherit(&mut self, out: Value, v: Value) -> Option<A::Reg> {
        let r = self.held(v)?;
        (self.def(out) == r).then_some(r)
    }

    /// A register for the instruction compiled to use apart from any value: the next of the
    /// point's scratch registers.
    ///
    /// # Panics
    /// Panics past the scratch registers the architecture said the instruction takes.
    pub fn scratch(&mut self) -> A::Reg {
        let temps = self.plan().temps(self.point);
        let p = *temps.get(self.temps).expect("an instruction takes no more scratch registers than it said");
        self.temps += 1;
        self.reg_of(p)
    }

    /// The bytes of `v`'s home.
    #[must_use]
    pub fn home_bytes(&self, v: Value) -> u64 {
        self.f.ty(v).bytes(self.unit.pointer_bits).max(A::WORD)
    }

    /// Whether `v` lives in its home alone, a value of the float bank (`Arch::bank`).
    #[must_use]
    pub fn homed(&self, v: Value) -> bool {
        A::bank(self.f.ty(v)) == Bank::Float
    }

    /// The number of uses `v` has.
    #[must_use]
    pub fn uses(&self, v: Value) -> u32 {
        self.values[v].uses
    }
}
