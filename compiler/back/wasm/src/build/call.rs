//! Calls: the arguments passed as the callee's signature passes them (`Passing`), a copy in the
//! caller's frame for a record passed by address, and the result taken back.

use std::borrow::Cow;

use vun_backend::{Lack, Missing};
use vun_lir::{CallKind, Callee, Ext, Value};
use vun_origin::OriginId;

use super::abi::{Given, Part, Passing};
use super::func::Lower;
use crate::module::op::{Num, Op};

impl Lower<'_> {
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        kind: CallKind,
        result: Option<Value>,
        at: OriginId,
    ) -> Result<(), Lack> {
        let passing: Cow<'_, Passing> = match callee {
            Callee::Func(f) => Cow::Borrowed(self.cx.passing(*f)),
            Callee::Import(i) => {
                Cow::Borrowed(self.cx.imported[*i].as_ref().expect("a called import is a function"))
            }
            Callee::Indirect(_, signature) => Cow::Owned(Passing::of(signature).map_err(|m| m.at(at))?),
        };
        if kind == CallKind::Tail && !passing.replaces() {
            return Err(Missing::TailCall.at(at));
        }
        let variadic = passing.varargs.as_ref().map_or(0, |v| v.spills.len());
        let (params, out) = args.split_at(passing.parts.len() + variadic);
        let (params, rest) = params.split_at(passing.parts.len());
        if passing.through() {
            self.get(out[0]);
        }
        for (&arg, part) in params.iter().zip(&passing.parts) {
            self.pass(arg, *part).map_err(|m| m.at(at))?;
        }
        if let Some(varargs) = &passing.varargs {
            self.spill(rest, varargs).map_err(|m| m.at(at))?;
        }
        if kind == CallKind::Tail {
            self.tail(callee, &passing);
            return Ok(());
        }
        match callee {
            Callee::Func(f) => self.op(Op::Call(self.cx.funcs[*f].expect("a called function is reached"))),
            Callee::Import(i) => self.op(Op::Call(self.cx.imports[*i])),
            Callee::Indirect(v, _) => {
                self.get(*v);
                let ty = self.cx.module.type_of(&passing.ty).expect("indirect signatures are listed first");
                self.op(Op::CallIndirect(ty));
            }
        }
        match passing.given {
            Given::Value(_, narrowed) => {
                if let Some((bits, Ext::Sign)) = narrowed {
                    self.mask(bits);
                }
                self.set(result.expect("a call giving a value defines one"));
            }
            Given::Scalar(one) => {
                let held = self.temp(one.held);
                self.op(Op::LocalSet(held));
                self.get(out[0]);
                self.op(Op::I32Const(Self::i32(one.offset)));
                self.op(Op::Num(Num::I32Add));
                self.op(Op::LocalGet(held));
                self.op(Op::Store(Self::mem(one.ty).map_err(|m| m.at(at))?));
            }
            Given::None | Given::Through => {}
        }
        Ok(())
    }

    /// The call after its arguments are passed: the frame given back first, then a jump.
    fn tail(&mut self, callee: &Callee, passing: &Passing) {
        if let Callee::Indirect(v, _) = callee {
            self.get(*v);
        }
        self.release_frame();
        match callee {
            Callee::Func(f) => {
                self.op(Op::ReturnCall(self.cx.funcs[*f].expect("a called function is reached")));
            }
            Callee::Import(i) => self.op(Op::ReturnCall(self.cx.imports[*i])),
            Callee::Indirect(..) => {
                let ty = self.cx.module.type_of(&passing.ty).expect("indirect signatures are listed first");
                self.op(Op::ReturnCallIndirect(ty));
            }
        }
    }

    fn pass(&mut self, arg: Value, part: Part) -> Result<(), Missing> {
        match part {
            Part::Value(_, narrowed) => {
                self.get(arg);
                if let Some((bits, Ext::Sign)) = narrowed {
                    self.extend(bits);
                }
            }
            Part::Scalar(one) => {
                self.get(arg);
                self.load_at(one)?;
            }
            Part::Copy { size, align } => {
                let area = self.alloc(size, align);
                self.frame_addr(area);
                self.get(arg);
                self.op(Op::I32Const(Self::i32(size)));
                self.op(Op::MemoryCopy);
                self.frame_addr(area);
            }
        }
        Ok(())
    }
}
