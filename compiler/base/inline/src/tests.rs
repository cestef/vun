use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::{Base, Block, Bound, Disp, FormId, Inst, Mem, OpId, Operand, Rebind, SizeId};

/// `lea {out r}, [{in a} + 3]`, bindings named by numbers: `a` is 1, `r` is 2.
fn lea() -> Block<u32, u32, u32> {
    let file = SourceFiles::new().add("a.vun", "lea");
    let origin = OriginTable::new().source(Span::new(file, 0, 3));
    let a = Base::Bound(Bound::In { value: 1, reg: None });
    let address = Mem { base: Some(a), index: None, disp: Disp::Bytes(3) };
    let operands = vec![Operand::Bound(Bound::Out { place: 2, reg: None }), Operand::Mem(address)];
    let inst = Inst { op: OpId(7), form: FormId(0), size: SizeId(3), operands, origin };
    let arch = vun_items::lang::arch_x86_64::arith.parent().expect("a namespace");
    Block { arch, insts: vec![inst], returns: true }
}

/// Values become strings, places their negation, symbols stay.
struct Shown;

impl Rebind<u32, u32, u32> for Shown {
    type Value = String;
    type Place = i64;
    type Symbol = u32;
    type Error = std::convert::Infallible;

    fn value(&mut self, value: u32) -> Result<String, Self::Error> {
        Ok(value.to_string())
    }

    fn place(&mut self, place: u32) -> Result<i64, Self::Error> {
        Ok(-i64::from(place))
    }

    fn symbol(&mut self, symbol: u32) -> Result<u32, Self::Error> {
        Ok(symbol)
    }
}

#[test]
fn a_block_rebinds_its_values_in_order_and_keeps_its_instructions() {
    let Ok(rebound) = lea().rebind(&mut Shown);
    let bindings: Vec<_> = rebound.bindings().cloned().collect();
    assert_eq!(
        bindings,
        [Bound::Out { place: -2, reg: None }, Bound::In { value: "1".to_owned(), reg: None }]
    );
    assert_eq!((rebound.insts[0].op, rebound.insts[0].size), (OpId(7), SizeId(3)));
}
