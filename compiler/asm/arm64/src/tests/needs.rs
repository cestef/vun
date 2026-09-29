//! What a processor needs to have each form at each width, with LLVM's assembler as the oracle:
//! of each form's sampled lines, the first a processor having every feature takes stands for it
//! (the samples hold some no processor does, a pair loaded into one register); one of Armv8.0
//! takes those needing nothing, and refuses each needing a feature (`lse`). The decoder test
//! checks every encoding.

use vun_harness::llvm::{Assembled, Assembler};
use vun_par::Parallel;

use crate::{Asm, Feature, Need, Op};

use super::{Sym, assemble, combinations, linked};

/// A processor, and the features it has.
struct Core {
    llvm: Assembler,
    features: &'static [Feature],
}

const EVERY: Core =
    Core { llvm: Assembler { args: &["--triple=aarch64", "-mattr=+lse"] }, features: &[Feature::Lse] };
const BASE: Core = Core { llvm: Assembler { args: &["--triple=aarch64", "-mattr=-lse"] }, features: &[] };

/// A form at a width: its sampled lines, and what a processor needs to have it.
struct Form {
    lines: Vec<String>,
    needs: Vec<Need>,
}

impl Core {
    fn has(&self, form: &Form) -> bool {
        form.needs.iter().all(|&Need::Feature(feature)| self.features.contains(&feature))
    }

    /// Whether LLVM's assembler takes `lines` for it: `None` where the assembler cannot run.
    fn takes(&self, lines: &[&str]) -> Option<bool> {
        let text: String = lines.iter().flat_map(|line| [line, "\n"]).collect();
        Some(matches!(self.llvm.assemble(&text)?, Assembled::Object(_)))
    }
}

/// Every form at each width, with a line for each sampled combination of operands it takes
/// first.
fn forms() -> Vec<Form> {
    let label = Asm::<Sym>::default().label();
    let mut forms = Vec::new();
    for &op in Op::ALL {
        for form in op.forms().iter().filter(|f| !linked(f)) {
            for sized in form.sizes {
                let lines = combinations(form.args.iter().map(|a| a.kind), label)
                    .into_iter()
                    .filter(|operands| op.form(sized.size, operands) == Some(form))
                    .map(|operands| assemble(op, sized.size, &operands).1.to_string())
                    .collect();
                let needs = [form.requires, sized.requires].into_iter().flatten().collect();
                forms.push(Form { lines, needs });
            }
        }
    }
    forms
}

#[test]
fn armv8_0_takes_the_forms_needing_nothing_and_refuses_those_needing_a_feature() {
    let forms = forms();
    if EVERY.takes(&[]).is_none() {
        return;
    }
    // Each form's first line a processor having every feature takes.
    let taken = forms
        .par_map(|form| form.lines.iter().find(|line| EVERY.takes(&[line.as_str()]) == Some(true)).cloned());
    let taken: Vec<(&Form, String)> =
        forms.iter().zip(taken).filter_map(|(form, line)| Some((form, line?))).collect();
    let (plain, featured): (Vec<_>, Vec<_>) = taken.iter().partition(|(form, _)| BASE.has(form));
    assert!(!featured.is_empty(), "some forms need a feature");
    let plain: Vec<&str> = plain.iter().map(|(_, line)| line.as_str()).collect();
    assert_eq!(BASE.takes(&plain), Some(true), "Armv8.0 refuses a form needing nothing");
    let taken =
        featured.par_map(|(_, line)| (BASE.takes(&[line.as_str()]) == Some(true)).then(|| line.clone()));
    let taken: Vec<String> = taken.into_iter().flatten().collect();
    assert!(taken.is_empty(), "Armv8.0 takes forms needing a feature:\n{}", taken.join("\n"));
}
