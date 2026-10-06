//! The table's invariants: opcodes are unique, and what a form pops and pushes is as its operands
//! allow, and a branch names its block.

use std::collections::BTreeMap;

use crate::{Choice, Control, Feature, Imm, Kind, Op, Ty};

#[test]
fn opcodes_are_unique_to_an_instruction() {
    let mut seen = BTreeMap::new();
    for op in Op::ALL {
        for form in op.forms() {
            let first = seen.entry(form.base).or_insert(*op);
            assert_eq!(*first, *op, "{} repeats the opcode {:#x} of {}", op.name(), form.base, first.name());
        }
    }
}

#[test]
fn a_form_popping_or_pushing_its_local_binds_one() {
    for op in Op::ALL {
        for form in op.forms() {
            let uses = form.pops.iter().chain(form.pushes).any(|&ty| ty == Ty::Local);
            let binds = form.args.iter().any(|arg| arg.kind == Kind::Local);
            assert_eq!(uses, binds, "{}", op.name());
        }
    }
}

#[test]
fn a_branch_takes_depths_and_a_block_is_empty_or_of_a_type() {
    let depth = [Kind::Imm(Imm::Unsigned(32))];
    let typed = [Kind::Choice(Choice::Yields)];
    for op in Op::ALL {
        for form in op.forms() {
            let kinds: Vec<_> = form.args.iter().map(|arg| arg.kind).collect();
            match form.control {
                Control::Branch | Control::BranchIf => assert_eq!(kinds, depth, "{}", op.name()),
                Control::BranchTable => assert_eq!(kinds, [Kind::Depths], "{}", op.name()),
                Control::Call => assert_eq!(kinds, [Kind::Symbol], "{}", op.name()),
                Control::Open | Control::OpenLoop | Control::OpenIf => {
                    // Without a type the block's byte follows its opcode, with one the type does.
                    assert!(
                        (kinds.is_empty() && form.tail == [0x40]) || (kinds == typed && form.tail.is_empty()),
                        "{}",
                        op.name()
                    );
                }
                Control::Reopen | Control::Close | Control::None => {}
            }
        }
    }
}

#[test]
fn every_vector_instruction_needs_the_feature_and_no_other_does() {
    for op in Op::ALL {
        for form in op.forms() {
            let needs = matches!(form.requires, Some(vun_isa::Need::Feature(Feature::Simd128)));
            assert_eq!(needs, form.prefix() == Some(0xfd), "{}", op.name());
        }
    }
}
