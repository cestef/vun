use vun_index::{Idx, IndexVec};
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::place::Place;
use crate::remap::Remap;
use crate::verify::{Entity, Malformed};
use crate::{Block, Body, LocalDecl, Operand, Role, Rvalue, Stmt, Terminator, TerminatorKind};

/// `_0 = _1; return` with one parameter, and `edit` applied.
fn body(origins: &OriginTable, edit: impl FnOnce(&mut Body)) -> Body {
    let file = SourceFiles::new().add("a.vun", "f := (x: u8) -> u8 { x };");
    let origin = origins.source(Span::new(file, 5, 24));
    let mut locals = IndexVec::new();
    let ret = locals.push(LocalDecl { name: None, ty: None, role: Role::Held, origin });
    let param = locals.push(LocalDecl { name: None, ty: None, role: Role::Held, origin });
    let stmt = Stmt { place: Place::of(ret), value: Rvalue::Use(Operand::Place(Place::of(param))), origin };
    let mut blocks = IndexVec::new();
    blocks.push(Block { stmts: vec![stmt], term: Terminator { kind: TerminatorKind::Return, origin } });
    let mut body = Body { locals, params: vec![param], ret, blocks, origin, coroutine: false };
    edit(&mut body);
    body
}

#[test]
fn the_verifier_accepts_a_body_and_rejects_dangling_names() {
    let origins = OriginTable::new();
    assert_eq!(body(&origins, |_| {}).verify(&origins), Ok(()));
    let unknown = body(&origins, |b| b.blocks[Idx::new(0)].stmts[0].place = Place::of(Idx::new(7)));
    let at = Entity::Stmt { block: 0, index: 0 };
    assert_eq!(unknown.verify(&origins), Err(Malformed::UnknownLocal { at, local: 7 }));
    let jump = body(&origins, |b| b.blocks[Idx::new(0)].term.kind = TerminatorKind::Goto(Idx::new(3)));
    assert_eq!(jump.verify(&origins), Err(Malformed::UnknownBlock { at: Entity::Terminator(0), block: 3 }));
    let reused = body(&origins, |b| b.params.push(b.ret));
    assert_eq!(reused.verify(&origins), Err(Malformed::ParamReused(0)));
    let elsewhere = OriginTable::new();
    assert_eq!(body(&origins, |_| {}).verify(&elsewhere), Err(Malformed::NoOrigin(Entity::Body)));
}

#[test]
fn the_verifier_checks_what_an_assembly_block_writes_and_where_it_ends() {
    use vun_inline::{Block as Asm, Bound, FormId, Inst, OpId, Operand as Bind, SizeId};
    let origins = OriginTable::new();
    // Local `out` written by a block, which returns or not, before `term`.
    let verify = |out: usize, returns: bool, term: TerminatorKind| {
        let body = body(&origins, |b| {
            let block = &mut b.blocks[Idx::new(0)];
            let origin = block.stmts[0].origin;
            let operands = vec![Bind::Bound(Bound::Out { place: Place::of(Idx::new(out)), reg: None })];
            let inst = Inst { op: OpId(0), form: FormId(0), size: SizeId(0), operands, origin };
            let arch = vun_items::lang::arch_x86_64::arith.parent().expect("a namespace");
            block.stmts[0].value =
                Rvalue::Asm(Box::new(Asm { arch, insts: vec![inst], returns, needs: Vec::new() }));
            block.term.kind = term;
        });
        body.verify(&origins)
    };
    assert_eq!(verify(0, true, TerminatorKind::Return), Ok(()));
    assert_eq!(verify(0, false, TerminatorKind::Unreachable), Ok(()));
    assert_eq!(verify(0, false, TerminatorKind::Return), Err(Malformed::PastNever(0)));
    let at = Entity::Stmt { block: 0, index: 0 };
    assert_eq!(verify(7, true, TerminatorKind::Return), Err(Malformed::UnknownLocal { at, local: 7 }));
}

#[test]
fn remapping_renumbers_what_an_assembly_block_writes() {
    use vun_inline::{Block as Asm, Bound, FormId, Inst, OpId, Operand as Bind, SizeId};
    let origins = OriginTable::new();
    let mut body = body(&origins, |b| {
        let block = &mut b.blocks[Idx::new(0)];
        let origin = block.stmts[0].origin;
        let operands = vec![Bind::Bound(Bound::Out { place: Place::of(Idx::new(1)), reg: None })];
        let inst = Inst { op: OpId(0), form: FormId(0), size: SizeId(0), operands, origin };
        let arch = vun_items::lang::arch_x86_64::arith.parent().expect("a namespace");
        block.stmts[0].value =
            Rvalue::Asm(Box::new(Asm { arch, insts: vec![inst], returns: true, needs: Vec::new() }));
    });
    let remap = Remap {
        locals: IndexVec::from(vec![Idx::new(4), Idx::new(5)]),
        blocks: IndexVec::from(vec![Idx::new(0)]),
    };
    let stmt = &mut body.blocks[Idx::new(0)].stmts[0];
    remap.stmt(stmt);
    assert_eq!(stmt.value.outputs(), vec![(&Place::of(Idx::new(5)), false)]);
}

/// `t = (); b = t; _0 = b` with the temporary `t` read as many times as `reads` says; `forward`
/// applied.
fn forwarded(origins: &OriginTable, reads: usize) -> Body {
    use vun_intern::Symbol;
    use vun_sema_ty::TyKind;

    use crate::Aggregate;

    let mut body = body(origins, |_| {});
    let origin = body.origin;
    let unit = Some(TyKind::unit());
    let temp = body.locals.push(LocalDecl { name: None, ty: unit, role: Role::Held, origin });
    let bound =
        body.locals.push(LocalDecl { name: Some(Symbol::intern("b")), ty: unit, role: Role::Held, origin });
    let stmt = |place: Place, value| Stmt { place, value, origin };
    let read = |local| Rvalue::Use(Operand::Place(Place::of(local)));
    let mut stmts = vec![
        stmt(Place::of(temp), Rvalue::Aggregate { kind: Aggregate::Tuple, parts: Vec::new() }),
        stmt(Place::of(bound), read(temp)),
    ];
    stmts.extend((1..reads).map(|_| stmt(Place::of(body.ret), read(temp))));
    body.blocks[Idx::new(0)].stmts = stmts;
    body.forward();
    body
}

#[test]
fn a_temporary_moved_into_a_binding_is_built_there() {
    let origins = OriginTable::new();
    let alone = forwarded(&origins, 1);
    let stmts = &alone.blocks[Idx::new(0)].stmts;
    assert_eq!(stmts[0].place.local, Idx::new(3));
    assert_eq!(stmts[1].value, Rvalue::Dead);
    let read_again = forwarded(&origins, 2);
    let stmts = &read_again.blocks[Idx::new(0)].stmts;
    assert_eq!(stmts[0].place.local, Idx::new(2));
    assert_ne!(stmts[1].value, Rvalue::Dead);
}
