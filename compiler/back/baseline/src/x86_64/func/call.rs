//! Calls: every value still used is stored home, since a call may write any register values
//! are held in; arguments go where the callee's convention passes them, and a result comes back
//! in registers or through memory.

use vun_backend::Lack;
use vun_lir::{AbiType, Address, Callee, Conv, Ext, Signature, Value};

use super::super::abi::{Carrier, Frame, Loc, Ret};
use super::super::encode::{Alu, Shift};
use super::super::{Mem, Reg, Size};
use super::FnCompiler;

impl FnCompiler<'_> {
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        result: Option<Value>,
    ) -> Result<(), Lack> {
        let signature: &Signature = match callee {
            Callee::Func(f) => &self.unit.functions[*f].signature,
            Callee::Import(i) => match &self.unit.imports[*i].kind {
                vun_lir::ImportKind::Func(s) => s,
                vun_lir::ImportKind::Data => unreachable!("the verifier checks calls call functions"),
            },
            Callee::Indirect(_, s) => s,
        };
        let frame = Frame::of(signature).map_err(|m| self.lack(m))?;
        for &a in args {
            self.size(a)?;
        }
        self.spill_live();
        self.clear();
        self.outgoing = self.outgoing.max(frame.stack);
        // Arguments on the stack and records in registers go through the scratch register, so
        // they come before those passed in registers.
        for (&a, loc) in args.iter().zip(&frame.params) {
            match loc {
                Loc::Stack(offset) => {
                    self.get_into(a, Reg::SCRATCH);
                    self.asm.store(Size::B64, Self::outgoing(*offset), Reg::SCRATCH);
                }
                Loc::Copied { offset, size } => {
                    self.get_into(a, Reg::SCRATCH);
                    self.move_bytes(Self::outgoing(*offset), Mem::at(Reg::SCRATCH, 0), *size);
                }
                Loc::Reg(_) | Loc::Split(_) | Loc::Returned => {}
            }
        }
        for (i, (&a, loc)) in args.iter().zip(&frame.params).enumerate() {
            match loc {
                Loc::Reg(carrier) => {
                    let bytes = u64::from(self.size(a)?.bytes());
                    self.pass(a, *carrier, bytes);
                    if let (Some(AbiType::Int(scalar, ext)), Carrier::Int(r)) =
                        (signature.params.get(i), carrier)
                    {
                        self.extend_int(*r, self.size_of(*scalar)?, *ext);
                    }
                }
                Loc::Split(parts) => {
                    self.get_into(a, Reg::SCRATCH);
                    for (k, &(carrier, bytes)) in parts.iter().enumerate() {
                        let at = Mem::at(Reg::SCRATCH, i32::try_from(k * 8).expect("two eightbytes"));
                        self.load_part(carrier, at, bytes);
                    }
                }
                Loc::Stack(_) | Loc::Copied { .. } | Loc::Returned => {}
            }
        }
        // A variadic C function reads how many SSE registers carry arguments from `al`.
        if signature.conv == Conv::C {
            self.asm.mov_ri(Reg::Rax, u64::from(frame.sse));
        }
        match callee {
            Callee::Func(f) => self.asm.call(Address::Func(*f)),
            Callee::Import(i) => self.asm.call(Address::Import(*i)),
            Callee::Indirect(target, _) => {
                self.get_into(*target, Reg::SCRATCH);
                self.asm.call_reg(Reg::SCRATCH);
            }
        }
        match &frame.ret {
            Ret::Reg(carrier) => {
                if let Some(out) = result {
                    let bytes = u64::from(self.size(out)?.bytes());
                    self.receive(out, *carrier, bytes);
                }
            }
            Ret::Split(parts) => {
                let out = *args.last().expect("the address of the result is passed last");
                self.get_into(out, Reg::SCRATCH);
                for (k, &(carrier, bytes)) in parts.iter().enumerate() {
                    let at = Mem::at(Reg::SCRATCH, i32::try_from(k * 8).expect("two eightbytes"));
                    self.store_part(at, carrier, bytes);
                }
            }
            Ret::None | Ret::Hidden => {}
        }
        Ok(())
    }

    /// Where the stack argument at `offset` goes: the bottom of the frame.
    fn outgoing(offset: u64) -> Mem {
        Mem::at(Reg::Rsp, i32::try_from(offset).expect("arguments fit 2 GiB"))
    }

    /// Extends an integer of `size` in `r` to 32 bits as the C convention expects of callers.
    pub(super) fn extend_int(&mut self, r: Reg, size: Size, ext: Ext) {
        if size < Size::B32 {
            match ext {
                Ext::Zero => self.asm.zero_extend(size, r, r),
                Ext::Sign => self.asm.sign_extend(size, r, r),
            }
        }
    }

    /// Loads `bytes` (1 to 8) bytes at `at` into `dst`, in pieces a load can take.
    pub(super) fn load_bytes(&mut self, dst: Reg, at: Mem, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            self.asm.load(size, dst, at);
            return;
        }
        self.asm.mov_ri(dst, 0);
        for (size, offset) in Self::pieces(bytes) {
            self.asm.load(size, Reg::TEMP, at.plus(offset));
            self.asm.shift_imm(Shift::Shl, Size::B64, Reg::TEMP, offset * 8);
            self.asm.alu(Alu::Or, Size::B64, dst, Reg::TEMP);
        }
    }

    /// Stores the low `bytes` (1 to 8) bytes of `src` at `at`.
    pub(super) fn store_bytes(&mut self, at: Mem, src: Reg, bytes: u64) {
        if let Some(size) = Size::of_bytes(bytes) {
            self.asm.store(size, at, src);
            return;
        }
        self.asm.mov_rr(Reg::TEMP, src);
        for (size, offset) in Self::pieces(bytes) {
            self.asm.mov_rr(Reg::SCRATCH, Reg::TEMP);
            self.asm.shift_imm(Shift::Shr, Size::B64, Reg::SCRATCH, offset * 8);
            self.asm.store(size, at.plus(offset), Reg::SCRATCH);
        }
    }

    /// The loads of 4, 2 and 1 bytes covering `bytes` bytes, by their offsets.
    fn pieces(bytes: u64) -> Vec<(Size, i32)> {
        let mut done = 0;
        let mut pieces = Vec::new();
        for size in [Size::B32, Size::B16, Size::B8] {
            let n = u64::from(size.bytes());
            if bytes - done >= n {
                pieces.push((size, i32::try_from(done).expect("under 8")));
                done += n;
            }
        }
        pieces
    }

    /// Copies `bytes` bytes from `from` to `to` through `Reg::TEMP`.
    fn move_bytes(&mut self, to: Mem, from: Mem, bytes: u64) {
        let mut done = 0;
        for size in [Size::B64, Size::B32, Size::B16, Size::B8] {
            let n = u64::from(size.bytes());
            while bytes - done >= n {
                let at = i32::try_from(done).expect("records fit 2 GiB");
                self.asm.load(size, Reg::TEMP, from.plus(at));
                self.asm.store(size, to.plus(at), Reg::TEMP);
                done += n;
            }
        }
    }
}
