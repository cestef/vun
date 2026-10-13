//! The module verifier: every index names what it must, and every body passes the validation
//! algorithm of the WebAssembly specification (its appendix), over a stack of operand types and
//! one of control frames, the stack of an unreachable frame's rest being any types.

mod asm;
mod control;

use vun_isa::Nest;

use super::invalid::{Invalid, Why};
use super::op::Op;
use super::{Func, Kind, Linker, Module, SymbolId, TypeId, ValType};

/// A block being validated: the height of the operand stack when it began, whether an `if` or a
/// loop opened it, the values it gives, and whether the rest of it is unreachable.
struct Frame {
    height: usize,
    nest: Nest,
    yields: Vec<ValType>,
    unreachable: bool,
}

struct Check<'m> {
    module: &'m Module,
    locals: Vec<ValType>,
    results: &'m [ValType],
    stack: Vec<ValType>,
    frames: Vec<Frame>,
}

impl Module {
    /// Checks the module.
    ///
    /// # Errors
    /// Returns the first thing that makes it invalid.
    pub fn verify(&self) -> Result<(), Invalid> {
        for (i, f) in self.funcs.iter().enumerate() {
            self.verify_func(f).map_err(|(at, why)| Invalid { func: i, at, why })?;
        }
        Ok(())
    }

    fn verify_func(&self, f: &Func) -> Result<(), (usize, Why)> {
        let ty = self.types.get(f.ty).ok_or((0, Why::Type(f.ty)))?;
        let locals = ty.params.iter().chain(&f.locals).copied().collect();
        let mut check =
            Check { module: self, locals, results: &ty.results, stack: Vec::new(), frames: Vec::new() };
        check.frames.push(Frame { height: 0, nest: Nest::Plain, yields: Vec::new(), unreachable: false });
        for (at, op) in f.body.iter().enumerate() {
            check.op(op).map_err(|why| (at, why))?;
        }
        check.end_function().map_err(|why| (f.body.len(), why))?;
        if check.frames.is_empty() { Ok(()) } else { Err((f.body.len(), Why::Unclosed)) }
    }
}

impl Check<'_> {
    fn frame(&self) -> &Frame {
        self.frames.last().expect("the function's frame is open")
    }

    fn push(&mut self, ty: ValType) {
        self.stack.push(ty);
    }

    /// Pops an operand of type `ty`; in an unreachable rest, any operand not there is one.
    fn pop(&mut self, ty: ValType) -> Result<(), Why> {
        let frame = self.frame();
        if self.stack.len() == frame.height {
            return if frame.unreachable { Ok(()) } else { Err(Why::Underflow) };
        }
        match self.stack.pop() {
            Some(found) if found == ty => Ok(()),
            Some(found) => Err(Why::Mismatch { expected: ty, found }),
            None => Err(Why::Underflow),
        }
    }

    fn pop_all(&mut self, tys: &[ValType]) -> Result<(), Why> {
        tys.iter().rev().try_for_each(|&t| self.pop(t))
    }

    /// Pops any operand, giving its type when it is known.
    fn pop_any(&mut self) -> Result<Option<ValType>, Why> {
        let frame = self.frame();
        if self.stack.len() == frame.height {
            return if frame.unreachable { Ok(None) } else { Err(Why::Underflow) };
        }
        Ok(self.stack.pop())
    }

    fn unreachable(&mut self) {
        let height = self.frame().height;
        self.stack.truncate(height);
        self.frames.last_mut().expect("the function's frame is open").unreachable = true;
    }

    fn local(&self, l: u32) -> Result<ValType, Why> {
        self.locals.get(l as usize).copied().ok_or(Why::Local(l))
    }

    fn call(&mut self, ty: TypeId) -> Result<(), Why> {
        let ty = self.module.types.get(ty).ok_or(Why::Type(ty))?;
        let (params, results) = (ty.params.clone(), ty.results.clone());
        self.pop_all(&params)?;
        for r in results {
            self.push(r);
        }
        Ok(())
    }

    /// A call of a function of type `ty` that returns the caller's results: its own must be them.
    fn returning(&mut self, ty: TypeId) -> Result<(), Why> {
        let found = self.module.types.get(ty).ok_or(Why::Type(ty))?;
        if found.results != self.results {
            return Err(Why::Type(ty));
        }
        self.call(ty)?;
        self.pop_all(self.results)?;
        self.unreachable();
        Ok(())
    }

    fn symbol(&self, sym: SymbolId) -> Result<&Kind, Why> {
        self.module.symbols.get(sym).map(|s| &s.kind).ok_or(Why::Symbol(sym))
    }

    fn op(&mut self, op: &Op) -> Result<(), Why> {
        match op {
            Op::Unreachable => self.unreachable(),
            Op::Block => self.open(Nest::Plain, Vec::new()),
            Op::Loop => self.open(Nest::Loop, Vec::new()),
            Op::If => {
                self.pop(ValType::I32)?;
                self.open(Nest::If, Vec::new());
            }
            Op::Else => self.else_()?,
            Op::End => self.end_block()?,
            Op::Br(l) => self.jump(None, *l)?,
            Op::BrIf(l) => self.br_if(*l)?,
            Op::BrTable { cases, default } => self.jump(Some(cases), *default)?,
            Op::Return => {
                self.pop_all(self.results)?;
                self.unreachable();
            }
            Op::Call(sym) => self.call_symbol(*sym)?,
            Op::CallIndirect(ty) => {
                self.module.linker(Linker::Table).ok_or(Why::Type(*ty))?;
                self.pop(ValType::I32)?;
                self.call(*ty)?;
            }
            Op::ReturnCall(sym) => match self.symbol(*sym)? {
                Kind::Func { ty, .. } => self.returning(*ty)?,
                _ => return Err(Why::Symbol(*sym)),
            },
            Op::ReturnCallIndirect(ty) => {
                self.module.linker(Linker::Table).ok_or(Why::Type(*ty))?;
                self.pop(ValType::I32)?;
                self.returning(*ty)?;
            }
            Op::Drop => {
                self.pop_any()?;
            }
            Op::Select => self.select()?,
            Op::LocalGet(l) => self.push(self.local(*l)?),
            Op::LocalSet(l) => self.pop(self.local(*l)?)?,
            Op::LocalTee(l) => {
                let ty = self.local(*l)?;
                self.pop(ty)?;
                self.push(ty);
            }
            Op::GlobalGet(sym) => {
                self.stack_pointer(*sym)?;
                self.push(ValType::I32);
            }
            Op::GlobalSet(sym) => {
                self.stack_pointer(*sym)?;
                self.pop(ValType::I32)?;
            }
            Op::Load(mem) => {
                mem.accesses().ok_or(Why::Access(*mem))?;
                self.pop(ValType::I32)?;
                self.push(mem.ty);
            }
            Op::Store(mem) => {
                mem.accesses().ok_or(Why::Access(*mem))?;
                self.pop_all(&[ValType::I32, mem.ty])?;
            }
            Op::MemoryCopy => self.pop_all(&[ValType::I32; 3])?,
            Op::MemoryGrow => {
                self.pop_all(&[ValType::I32])?;
                self.push(ValType::I32);
            }
            Op::I32Const(_) => self.push(ValType::I32),
            Op::I64Const(_) => self.push(ValType::I64),
            Op::F32Const(_) => self.push(ValType::F32),
            Op::F64Const(_) => self.push(ValType::F64),
            Op::Address(sym, addend) => self.address(*sym, *addend)?,
            Op::Num(n) => {
                self.pop_all(n.pops())?;
                self.push(n.pushes());
            }
            Op::Asm(asm) => self.asm(asm)?,
        }
        Ok(())
    }

    fn call_symbol(&mut self, sym: SymbolId) -> Result<(), Why> {
        match self.symbol(sym)? {
            Kind::Func { ty, .. } => self.call(*ty),
            _ => Err(Why::Symbol(sym)),
        }
    }

    fn select(&mut self) -> Result<(), Why> {
        self.pop(ValType::I32)?;
        let second = self.pop_any()?;
        let first = self.pop_any()?;
        match (first, second) {
            (Some(a), Some(b)) if a != b => return Err(Why::Mismatch { expected: a, found: b }),
            (Some(t), _) | (None, Some(t)) => self.push(t),
            (None, None) => {}
        }
        Ok(())
    }

    fn stack_pointer(&self, sym: SymbolId) -> Result<(), Why> {
        if *self.symbol(sym)? == Kind::Linker(Linker::StackPointer) { Ok(()) } else { Err(Why::Symbol(sym)) }
    }

    /// The address of a symbol's function or data; a function's has no addend.
    fn address(&mut self, sym: SymbolId, addend: i32) -> Result<(), Why> {
        let fits = match self.symbol(sym)? {
            Kind::Func { .. } => addend == 0,
            Kind::Data(_) => true,
            Kind::Linker(_) => false,
        };
        if !fits {
            return Err(Why::Symbol(sym));
        }
        self.push(ValType::I32);
        Ok(())
    }
}
