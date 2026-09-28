//! Every form of the table, over operands covering its encodings' cases (low and high registers,
//! `sp` and `pc`, offsets and immediates at their bounds and past them), printed and assembled by
//! LLVM's assembler: the encoding is the one LLVM gives the printed text. Branches and symbols,
//! whose fields are written later, are checked by `links`.

mod links;
mod operands;

use std::io::Write;
use std::process::{Command, Stdio};

use vun_isa_thumb::{Form, Halves, Kind, Op, Operand, Size};

use crate::{Asm, Emit};
use operands::candidates;

type Sym = &'static str;

/// LLVM's assembler, for a core of Armv7E-M with the double-precision floating-point extension.
const LLVM_MC: &str = "llvm-mc";
const ARGS: [&str; 4] = ["--triple=thumbv7em-none-eabihf", "-mcpu=cortex-m7", "--show-encoding", "-"];

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

/// The encoding LLVM's assembler gives each line of `text`, or its errors.
fn assemble(text: &str) -> Option<Result<Vec<Vec<u8>>, String>> {
    let mut child = Command::new(LLVM_MC)
        .args(ARGS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(text.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    let errors = String::from_utf8_lossy(&out.stderr).into_owned();
    if !errors.is_empty() {
        return Some(Err(errors));
    }
    let encodings = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            line.split_once("encoding: [").map(|(_, bytes)| bytes.trim_end_matches(']').to_owned())
        })
        .map(|bytes| {
            bytes
                .split(',')
                .map(|b| u8::from_str_radix(b.trim().trim_start_matches("0x"), 16).expect("a byte"))
                .collect()
        })
        .collect();
    Some(Ok(encodings))
}

#[test]
fn every_form_assembles_as_llvm_does() {
    let mut lines = Vec::new();
    for &op in Op::ALL {
        for form in op.forms().iter().filter(|f| !linked(f)) {
            for sized in form.sizes {
                for operands in fitting(op, form, sized.size) {
                    let mut asm = Asm::<Sym>::listed();
                    asm.emit_by(op, form, sized.size, &operands).expect("the form takes them");
                    let code = asm.finish().expect("no label");
                    let inst = vun_isa_thumb::Inst::new(op, sized.size, &operands);
                    assert_eq!(code.bytes.len() as u64, form.bytes(), "{}", inst.by(form));
                    lines.push((inst.by(form).to_string(), code.bytes));
                }
            }
        }
        assert!(
            op.forms().iter().any(linked)
                || op.forms().iter().all(|f| f.sizes.iter().any(|s| !fitting(op, f, s.size).is_empty())),
            "{op:?} has a form no operand fits"
        );
    }
    let text: String = lines.iter().flat_map(|(line, _)| [line.as_str(), "\n"]).collect();
    let Some(assembled) = assemble(&text) else { return };
    let encodings = assembled.unwrap_or_else(|errors| panic!("LLVM refuses the text:\n{errors}"));
    assert_eq!(encodings.len(), lines.len());
    let differ: Vec<String> = lines
        .iter()
        .zip(&encodings)
        .filter(|((_, ours), theirs)| ours != *theirs)
        .map(|((line, ours), theirs)| format!("{line}  |  ours {ours:02x?}, LLVM's {theirs:02x?}"))
        .collect();
    assert!(
        differ.is_empty(),
        "{} of {} differ:\n{}",
        differ.len(),
        lines.len(),
        differ.iter().take(60).cloned().collect::<Vec<_>>().join("\n")
    );
}
