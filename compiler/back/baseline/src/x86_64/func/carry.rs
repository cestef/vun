//! Values between where they are held, general registers and memory, and the carriers a
//! convention passes them in: a general register as it is, an SSE register by its bits.

use vun_lir::Value;

use super::super::abi::Carrier;
use super::super::{Mem, Reg, Size};
use super::FnCompiler;

/// The size of a float's bits of `bytes` bytes, or of an eightbyte's part.
fn bits(bytes: u64) -> Size {
    if bytes > 4 { Size::B64 } else { Size::B32 }
}

impl FnCompiler<'_> {
    /// Puts `v`, of `bytes` bytes, in `carrier`.
    pub(super) fn pass(&mut self, v: Value, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.get_into(v, r),
            Carrier::Sse(x) => {
                self.get_into(v, Reg::SCRATCH);
                self.asm.to_xmm(bits(bytes), x, Reg::SCRATCH);
            }
        }
    }

    /// Takes `v`, of `bytes` bytes, from `carrier`, holding it in a general register.
    pub(super) fn receive(&mut self, v: Value, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.bind(v, r, false),
            Carrier::Sse(x) => {
                let d = self.def(v);
                self.asm.from_xmm(bits(bytes), d, x);
            }
        }
    }

    /// Loads the `bytes` bytes at `at` into `carrier`.
    pub(super) fn load_part(&mut self, carrier: Carrier, at: Mem, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.load_bytes(r, at, bytes),
            Carrier::Sse(x) => {
                self.load_bytes(Reg::TEMP, at, bytes);
                self.asm.to_xmm(bits(bytes), x, Reg::TEMP);
            }
        }
    }

    /// Stores the low `bytes` bytes `carrier` holds at `at`.
    pub(super) fn store_part(&mut self, at: Mem, carrier: Carrier, bytes: u64) {
        match carrier {
            Carrier::Int(r) => self.store_bytes(at, r, bytes),
            Carrier::Sse(x) => {
                self.asm.from_xmm(bits(bytes), Reg::TEMP, x);
                self.store_bytes(at, Reg::TEMP, bytes);
            }
        }
    }
}
