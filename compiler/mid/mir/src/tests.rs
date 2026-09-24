use vun_index::{Idx, IndexVec};
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::place::Place;
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
    let mut body = Body { locals, params: vec![param], ret, blocks, origin };
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
