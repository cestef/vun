//! Memory: loads and stores, whose addresses fold a slot's and constant offsets where a form
//! reaches them; ordered accesses, `ldar` and `stlr`, which together are sequentially
//! consistent; copies, through a register or in a loop.

use vun_asm_arm64::{Cc, Emit, Mem, Op, Operand, Size};
use vun_backend::Lack;
use vun_codegen::Arch;
use vun_lir::{Access, InstKind, Order, Value};

use super::Sel;
use crate::arm64::Arm;
use crate::arm64::snippets::{ADD, LOAD, LOAD_ACQUIRE, STORE, STORE_RELEASE};

/// Copies up to this many bytes move through a register, larger ones in a loop.
const UNROLLED: u64 = 128;
/// The bytes a loop's step copies.
const STEP: u64 = 8;

impl Sel<'_, '_> {
    pub(super) fn memory(&mut self, kind: &InstKind, out: Option<Value>) -> Result<(), Lack> {
        match *kind {
            InstKind::Load { ty, addr, access } => {
                let size = self.size_of(ty)?;
                let snippet = if ordered(access) { LOAD_ACQUIRE } else { LOAD };
                self.apply(snippet, &[addr], out, size, None);
            }
            InstKind::Store { value, addr, access } => {
                let size = self.size(value)?;
                let snippet = if ordered(access) { STORE_RELEASE } else { STORE };
                self.apply(snippet, &[value, addr], None, size, None);
            }
            InstKind::Rmw { op, addr, value, .. } => {
                let size = self.size(value)?;
                self.rmw(op, addr, value, out, size);
            }
            InstKind::CompareExchange { addr, expected, new, .. } => {
                let size = self.size(new)?;
                self.compare_exchange(addr, expected, new, out, size);
            }
            InstKind::Offset { base, offset } => self.apply(ADD, &[base, offset], out, Size::B64, None),
            InstKind::Copy { to, from, bytes } => self.copy(to, from, bytes),
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => {
                unreachable!("recomputed where used")
            }
            InstKind::Binary { .. }
            | InstKind::Compare { .. }
            | InstKind::Cast { .. }
            | InstKind::Select { .. }
            | InstKind::Call { .. } => unreachable!("selected apart"),
        }
        Ok(())
    }

    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, in a loop of words.
    fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.reg(to), self.reg(from));
            self.copy_bytes(to, from, bytes, Arm::CARRIER);
            return;
        }
        let (dst, src, left) = (self.scratch(), self.scratch(), self.scratch());
        self.reg_to(to, dst);
        self.reg_to(from, src);
        let steps = i64::try_from(bytes / STEP).expect("copies fit memory");
        crate::arm64::constant(&mut self.asm, left, steps.cast_unsigned());
        let again = self.asm.label();
        self.asm.bind(again);
        let step = i64::try_from(STEP).expect("a word");
        let (read, written) = (Mem::Post { base: src, offset: step }, Mem::Post { base: dst, offset: step });
        self.asm.emit(Op::Ldr, Size::B64, &[Arm::CARRIER.into(), read.into()]);
        self.asm.emit(Op::Str, Size::B64, &[Arm::CARRIER.into(), written.into()]);
        self.asm.emit(Op::Subs, Size::B64, &[left.into(), left.into(), Operand::Imm(1)]);
        self.asm.emit(Op::Bcond, Size::B64, &[Cc::Ne.into(), again.into()]);
        self.copy_bytes(dst, src, bytes % STEP, Arm::CARRIER);
    }
}

/// Whether an access is ordered beyond a plain one: acquiring loads, releasing stores.
fn ordered(access: Access) -> bool {
    matches!(access, Access::Atomic(order) if order != Order::Relaxed)
}
