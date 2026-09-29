//! Processors of each profile, with and without a floating-point unit and division, as LLVM's
//! assembler assembles for them: what each has, the encodings of the lines it has what they need
//! for, and whether it refuses one it lacks something for.

use vun_harness::llvm::{Assembled, Assembler};
use vun_isa_thumb::{Feature, Need, Size};

use super::Line;

/// What LLVM's assembler makes of lines: the bytes its object's code holds, or a refusal.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Code {
    Encoded(Vec<u8>),
    Refused,
}

/// A processor, and what it has beyond Thumb.
pub(super) struct Core {
    pub(super) llvm: Assembler,
    features: &'static [Feature],
    /// The widths of the floats its floating-point unit computes.
    floats: &'static [Size],
}

pub(super) const CORES: [Core; 5] = [
    Core {
        llvm: Assembler { args: &["--triple=thumbv7m-none-eabi", "-mcpu=cortex-m3"] },
        features: &[Feature::Microcontroller, Feature::Divide],
        floats: &[],
    },
    Core {
        llvm: Assembler { args: &["--triple=thumbv7em-none-eabihf", "-mattr=+vfp4d16sp"] },
        features: &[Feature::Microcontroller, Feature::Divide],
        floats: &[Size::B32],
    },
    Core {
        llvm: Assembler { args: &["--triple=thumbv7em-none-eabihf", "-mcpu=cortex-m7"] },
        features: &[Feature::Microcontroller, Feature::Divide],
        floats: &[Size::B32, Size::B64],
    },
    Core {
        llvm: Assembler { args: &["--triple=thumbv7a-none-eabihf", "-mattr=+vfp3d16"] },
        features: &[Feature::Application],
        floats: &[Size::B32, Size::B64],
    },
    Core {
        llvm: Assembler { args: &["--triple=thumbv7a-none-eabihf", "-mattr=+vfp3d16,+hwdiv"] },
        features: &[Feature::Application, Feature::Divide],
        floats: &[Size::B32, Size::B64],
    },
];

impl Core {
    fn has(&self, need: Need) -> bool {
        match need {
            Need::Feature(feature) => self.features.contains(&feature),
            Need::Float(size) => self.floats.contains(&size),
        }
    }

    /// Whether it has everything `line` needs.
    pub(super) fn takes(&self, line: &Line) -> bool {
        line.needs.iter().all(|&need| self.has(need))
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
