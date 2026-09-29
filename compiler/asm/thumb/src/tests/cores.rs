//! Processors of each profile, with and without a floating-point unit and division, as LLVM's
//! assembler assembles for them: what each has, the encodings of the lines it has what they need
//! for, and whether it refuses one it lacks something for.

use vun_harness::llvm::{Assembled, Assembler};
use vun_isa_thumb::{Feature, Processor};
use vun_target::Floats;

use super::Line;

/// What LLVM's assembler makes of lines: the bytes its object's code holds, or a refusal.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Code {
    Encoded(Vec<u8>),
    Refused,
}

/// A processor, as LLVM's assembler and our table see it.
pub(super) struct Core {
    pub(super) llvm: Assembler,
    processor: Processor,
}

impl Core {
    /// Processors of each profile, with and without a floating-point unit and division.
    pub(super) fn all() -> [Self; 5] {
        let (none, single) = (Floats::default(), Floats::default().with(32).expect("a format"));
        let double = single.with(64).expect("a format");
        let core = |args, features: &[Feature], floats| Self {
            llvm: Assembler { args },
            processor: Processor::new(features.iter().copied(), floats),
        };
        let microcontroller = &[Feature::Microcontroller, Feature::Divide];
        [
            core(&["--triple=thumbv7m-none-eabi", "-mcpu=cortex-m3"], microcontroller, none),
            core(&["--triple=thumbv7em-none-eabihf", "-mattr=+vfp4d16sp"], microcontroller, single),
            core(&["--triple=thumbv7em-none-eabihf", "-mcpu=cortex-m7"], microcontroller, double),
            core(&["--triple=thumbv7a-none-eabihf", "-mattr=+vfp3d16"], &[Feature::Application], double),
            core(
                &["--triple=thumbv7a-none-eabihf", "-mattr=+vfp3d16,+hwdiv"],
                &[Feature::Application, Feature::Divide],
                double,
            ),
        ]
    }

    /// Whether it has everything `line` needs.
    pub(super) fn takes(&self, line: &Line) -> bool {
        self.processor.lacks(line.needs.iter().copied()).is_none()
    }

    /// What LLVM's assembler makes of `lines` for it; `None` where the assembler cannot run.
    pub(super) fn code(&self, lines: &[&Line]) -> Option<Code> {
        let text: String = lines.iter().flat_map(|line| [line.text.as_str(), "\n"]).collect();
        Some(match self.llvm.assemble(&text)? {
            Assembled::Refused => Code::Refused,
            Assembled::Object(bytes) => {
                let file = vun_elf::File::read(&bytes).expect("LLVM writes ELF");
                Code::Encoded(file.section(b".text").expect("code").bytes().to_vec())
            }
        })
    }
}
