use vun_index::Idx;
use vun_lir::testing::{origin, sum};
use vun_lir::{Address, FuncId, InstKind};
use vun_origin::OriginTable;

use crate::hoist::Hoist;

#[test]
fn constants_leave_the_loops_around_them_for_the_block_before() {
    let origins = OriginTable::new();
    let mut unit = sum(origin(&origins));
    let f = &mut unit.functions[FuncId::new(0)];
    let body = Idx::new(2);
    let one = f.blocks[body].insts[0];
    assert!(matches!(f.insts[one].kind, InstKind::Const { .. }));
    assert_eq!(f.hoist(), 1);
    assert!(f.blocks[Idx::new(0)].insts.contains(&one));
    assert!(!f.blocks[body].insts.contains(&one));
    assert_eq!(f.hoist(), 0);
    assert_eq!(unit.verify(&origins), Ok(()));
}

#[test]
fn the_address_of_a_symbol_leaves_the_loops_around_it() {
    let origins = OriginTable::new();
    let mut unit = sum(origin(&origins));
    let f = &mut unit.functions[FuncId::new(0)];
    let body = Idx::new(2);
    let address = f.blocks[body].insts[0];
    f.insts[address].kind = InstKind::Addr(Address::Func(FuncId::new(0)));
    assert_eq!(f.hoist(), 0);
    assert_eq!(f.hoist_addresses(), 1);
    assert!(f.blocks[Idx::new(0)].insts.contains(&address));
}
