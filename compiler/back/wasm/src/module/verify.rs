//! The module verifier: every index names what it must, and every body passes the validation
//! algorithm of the WebAssembly specification (its appendix), over a stack of operand types and
//! one of control frames, the stack of an unreachable frame's rest being any types.

use super::invalid::{Invalid, Why};
use super::op::Op;
use super::{Func, Kind, Linker, Module, SymbolId, TypeId, ValType};

/// A block being validated: the height of the operand stack when it began, whether an `if`
/// opened it, and whether the rest of it is unreachable.
struct Frame {
    height: usize,
    is_if: bool,
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
        check.frames.push(Frame { height: 0, is_if: false, unreachable: false });
        for (at, op) in f.body.iter().enumerate() {
            check.op(op).map_err(|why| (at, why))?;
        }
        check.end(true).map_err(|why| (f.body.len(), why))?;
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

    fn open(&mut self, is_if: bool) {
        self.frames.push(Frame { height: self.stack.len(), is_if, unreachable: false });
    }

    /// Ends the innermost block, which gives nothing, or the function's, which gives its results.
    fn end(&mut self, function: bool) -> Result<(), Why> {
        if function {
            if self.frames.len() != 1 {
                return Err(Why::Unclosed);
            }
            self.pop_all(self.results)?;
        }
        let frame = self.frames.pop().ok_or(Why::Unbalanced)?;
        if self.stack.len() != frame.height {
            return Err(Why::Leftover);
        }
        Ok(())
    }

    /// The operand types a branch to `label` carries: the function's results for its own frame.
    fn label(&self, label: u32) -> Result<&[ValType], Why> {
        let depth = label as usize;
        if depth >= self.frames.len() {
            return Err(Why::Label(label));
        }
        Ok(if depth == self.frames.len() - 1 { self.results } else { &[] })
    }

    fn branch(&mut self, label: u32) -> Result<(), Why> {
        let carried = self.label(label)?.to_vec();
        self.pop_all(&carried)?;
        Ok(())
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

    fn symbol(&self, sym: SymbolId) -> Result<&Kind, Why> {
        self.module.symbols.get(sym).map(|s| &s.kind).ok_or(Why::Symbol(sym))
    }

    fn op(&mut self, op: &Op) -> Result<(), Why> {
        match op {
            Op::Unreachable => self.unreachable(),
            Op::Block | Op::Loop => self.open(false),
            Op::If => {
                self.pop(ValType::I32)?;
                self.open(true);
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
                mem.codes().ok_or(Why::Access(*mem))?;
                self.pop(ValType::I32)?;
                self.push(mem.ty);
            }
            Op::Store(mem) => {
                mem.codes().ok_or(Why::Access(*mem))?;
                self.pop_all(&[ValType::I32, mem.ty])?;
            }
            Op::MemoryCopy => self.pop_all(&[ValType::I32; 3])?,
            Op::I32Const(_) => self.push(ValType::I32),
            Op::I64Const(_) => self.push(ValType::I64),
            Op::F32Const(_) => self.push(ValType::F32),
            Op::F64Const(_) => self.push(ValType::F64),
            Op::Address(sym, addend) => self.address(*sym, *addend)?,
            Op::Num(n) => {
                self.pop_all(n.pops())?;
                self.push(n.pushes());
            }
        }
        Ok(())
    }

    fn else_(&mut self) -> Result<(), Why> {
        if !self.frame().is_if {
            return Err(Why::Unbalanced);
        }
        self.end(false)?;
        self.open(false);
        Ok(())
    }

    fn end_block(&mut self) -> Result<(), Why> {
        if self.frames.len() == 1 {
            return Err(Why::Unbalanced);
        }
        self.end(false)
    }

    /// A branch to `default`, or, by an index, to one of a table's `cases`, which carry what it
    /// does.
    fn jump(&mut self, table: Option<&[u32]>, default: u32) -> Result<(), Why> {
        let cases = table.unwrap_or_default();
        if table.is_some() {
            self.pop(ValType::I32)?;
        }
        let arity = self.label(default)?.len();
        for &l in cases {
            if self.label(l)?.len() != arity {
                return Err(Why::Label(l));
            }
        }
        self.branch(default)?;
        self.unreachable();
        Ok(())
    }

    fn br_if(&mut self, label: u32) -> Result<(), Why> {
        self.pop(ValType::I32)?;
        let carried = self.label(label)?.to_vec();
        self.pop_all(&carried)?;
        for t in carried {
            self.push(t);
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
