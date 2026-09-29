//! Every form of the table, over operands covering its encodings' cases (low and high registers,
//! `sp` and `pc`, offsets and immediates at their bounds and past them), printed and assembled by
//! LLVM's assembler for processors of each profile: each processor encodes the lines it has what
//! they need for as the table does, read from the object it writes, and refuses each other line. Branches and symbols,
//! whose fields are written later, are checked by `links`.

mod cores;
mod links;
mod operands;

use vun_isa_thumb::{Cc, Form, Halves, Kind, Need, Op, Operand, Size, Then};

use vun_par::Parallel;

use crate::{Asm, Emit, Processor};
use cores::{Code, Core};
use operands::candidates;

type Sym = &'static str;

/// A line of text, its encoding, and what a processor needs to have its form.
struct Line {
    text: String,
    bytes: Vec<u8>,
    needs: Vec<Need>,
}

/// Each combination of operands whose first fitting form is `form`, once each.
fn fitting(op: Op, form: &'static Form, size: Size) -> Vec<Vec<Operand<Sym>>> {
    let mut all: Vec<Vec<Operand<Sym>>> = vec![Vec::new()];
    for kind in form.kinds() {
        let choices = candidates(kind);
        all = all
            .into_iter()
            .flat_map(|prefix| choices.iter().map(move |c| [&prefix[..], &[*c]].concat()))
            .collect();
    }
    all.into_iter().filter(|operands| op.form(size, operands) == Some(form)).collect()
}

/// Whether a form's fields are written later: a label's, or a symbol's.
fn linked(form: &Form) -> bool {
    form.kinds().any(|k| matches!(k, Kind::Target(_) | Kind::Low | Kind::High))
}

/// The instructions an `it` form makes conditional, as `nop`s holding on each condition: the
/// assembler refuses a block left short.
fn conditional(form: &Form, operands: &[Operand<Sym>]) -> Vec<Line> {
    let (Some(Kind::Then(then)), Some(&Operand::Cond(cc))) = (form.kinds().next(), operands.first()) else {
        return Vec::new();
    };
    let later: &[Cc] = match then {
        Then::None => &[],
        Then::T => &[cc],
        Then::E => &[cc.not()],
    };
    let mut nop = Asm::<Sym>::listed(Processor::every());
    nop.emit(Op::Nop, Size::B32, &[]);
    let bytes = nop.finish().expect("no label").bytes;
    [cc].iter()
        .chain(later)
        .map(|c| Line { text: format!("nop{c}"), bytes: bytes.clone(), needs: Vec::new() })
        .collect()
}

/// A line for each combination of operands of every form taking them first, with the
/// instructions an `it` form makes conditional.
fn lines() -> Vec<Line> {
    let mut lines = Vec::new();
    for &op in Op::ALL {
        for form in op.forms().iter().filter(|f| !linked(f)) {
            for sized in form.sizes {
                let needs: Vec<Need> = form.needs(sized.size).collect();
                for operands in fitting(op, form, sized.size) {
                    let mut asm = Asm::<Sym>::listed(Processor::every());
                    asm.emit_by(op, form, sized.size, &operands).expect("the form takes them");
                    let code = asm.finish().expect("no label");
                    let inst = vun_isa_thumb::Inst::new(op, sized.size, &operands);
                    assert_eq!(code.bytes.len() as u64, form.bytes(), "{}", inst.by(form));
                    lines.push(Line {
                        text: inst.by(form).to_string(),
                        bytes: code.bytes,
                        needs: needs.clone(),
                    });
                    lines.extend(conditional(form, &operands));
                }
            }
        }
        assert!(
            op.forms().iter().any(linked)
                || op.forms().iter().all(|f| f.sizes.iter().any(|s| !fitting(op, f, s.size).is_empty())),
            "{op:?} has a form no operand fits"
        );
    }
    lines
}

/// The lines of `taken` whose encodings differ from those `code` holds in order, with both.
fn differing(taken: &[&Line], code: &[u8]) -> Vec<String> {
    let mut at = 0;
    let mut differ = Vec::new();
    for line in taken {
        let theirs = code.get(at..at + line.bytes.len()).unwrap_or_default();
        if theirs != line.bytes {
            differ.push(format!("{}  |  ours {:02x?}, LLVM's {theirs:02x?}", line.text, line.bytes));
        }
        at += line.bytes.len();
    }
    differ
}

#[test]
fn every_form_assembles_as_llvm_does_where_a_processor_has_what_it_needs() {
    let lines = lines();
    for line in &lines {
        assert!(Core::all().iter().any(|core| core.takes(line)), "no processor has what {} needs", line.text);
    }
    for core in &Core::all() {
        let (taken, lacking): (Vec<&Line>, Vec<&Line>) = lines.iter().partition(|line| core.takes(line));
        let code = match core.code(&taken) {
            None => return,
            Some(Code::Encoded(code)) => code,
            // Refused together, each line refused alone is named.
            Some(Code::Refused) => {
                let refused = taken
                    .par_map(|line| (core.code(&[line]) == Some(Code::Refused)).then(|| line.text.clone()));
                let refused: Vec<String> = refused.into_iter().flatten().collect();
                panic!("{:?} refuses what it has what it needs for:\n{}", core.llvm.args, refused.join("\n"))
            }
        };
        let differ = differing(&taken, &code);
        assert!(
            differ.is_empty() && code.len() == taken.iter().map(|l| l.bytes.len()).sum::<usize>(),
            "{:?}: {} of {} differ:\n{}",
            core.llvm.args,
            differ.len(),
            taken.len(),
            differ.iter().take(60).cloned().collect::<Vec<_>>().join("\n")
        );
        let taken_alone = lacking
            .par_map(|line| matches!(core.code(&[line]), Some(Code::Encoded(_))).then(|| line.text.clone()));
        let taken_alone: Vec<String> = taken_alone.into_iter().flatten().collect();
        assert!(
            taken_alone.is_empty(),
            "{:?} takes what it lacks something for:\n{}",
            core.llvm.args,
            taken_alone.iter().take(60).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}
