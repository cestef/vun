//! Reading clang's assembly: which argument register or stack slot each result register of a
//! function is written from.

use super::Seen;
use crate::Carrier;

/// How a target's assembly is written.
#[derive(Clone, Copy)]
pub(super) enum Syntax {
    /// x86-64 in AT&T syntax: the destination last, the stack above the return address.
    Att,
    /// arm64: the destination first, the stack at `sp`.
    Arm64,
    /// 32-bit Arm as arm64, a double `d{k}` over the singles `s{2k}` and `s{2k+1}`.
    Arm32,
}

/// A result register: whether it is a floating-point one, and its position among the result
/// registers of its class.
pub(super) type Dest = (bool, u8);

/// The general registers of x86-64 by the names of each width, in the order System V passes
/// arguments in, then `rax`.
const X86_INTS: [[&str; 4]; 7] = [
    ["rdi", "edi", "di", "dil"],
    ["rsi", "esi", "si", "sil"],
    ["rdx", "edx", "dx", "dl"],
    ["rcx", "ecx", "cx", "cl"],
    ["r8", "r8d", "r8w", "r8b"],
    ["r9", "r9d", "r9w", "r9b"],
    ["rax", "eax", "ax", "al"],
];

impl Syntax {
    /// The register `name` names, as an argument register, and its position among the result
    /// registers of its class.
    fn register(self, name: &str) -> Option<(Carrier<u8, u8>, u8)> {
        let number = |prefixes: &[char]| {
            let rest = name.strip_prefix(prefixes)?;
            rest.parse::<u8>().ok()
        };
        match self {
            Syntax::Att => {
                if let Some(n) = name.strip_prefix("xmm").and_then(|n| n.parse().ok()) {
                    return Some((Carrier::Float(n), n));
                }
                let family = X86_INTS.iter().position(|names| names.contains(&name))?;
                let result = match family {
                    6 => 0,
                    2 => 1,
                    _ => u8::MAX,
                };
                Some((Carrier::Int(u8::try_from(family).ok()?), result))
            }
            Syntax::Arm32 => {
                let (carrier, n) = match number(&['r']) {
                    Some(n) => (Carrier::Int(n), n),
                    None => match number(&['d']) {
                        Some(n) => (Carrier::Float(2 * n), 2 * n),
                        None => number(&['s']).map(|n| (Carrier::Float(n), n))?,
                    },
                };
                Some((carrier, n))
            }
            Syntax::Arm64 => match number(&['x', 'w']) {
                Some(n) => Some((Carrier::Int(n), n)),
                None => number(&['d', 's', 'q', 'h', 'b', 'v']).map(|n| (Carrier::Float(n), n)),
            },
        }
    }

    /// The result register at `dest`, as an argument register.
    pub(super) fn result(self, (float, k): Dest) -> Carrier<u8, u8> {
        let name = match (self, float) {
            (_, true) => return Carrier::Float(k),
            (Syntax::Att, false) => ["rax", "rdx"][usize::from(k)],
            (Syntax::Arm64 | Syntax::Arm32, false) => return Carrier::Int(k),
        };
        self.register(name).expect("a register of the table").0
    }

    /// The stack offset of an argument the operand `memory` reads.
    fn stack(self, memory: &str) -> Option<u64> {
        match self {
            Syntax::Att => {
                let offset = memory.strip_suffix("(%rsp)")?;
                let offset: u64 = if offset.is_empty() { 0 } else { offset.parse().ok()? };
                offset.checked_sub(8)
            }
            Syntax::Arm64 | Syntax::Arm32 => {
                let inside = memory.strip_prefix("[sp")?.strip_suffix(']')?;
                let offset = inside.strip_prefix(", #").unwrap_or("0");
                offset.parse().ok()
            }
        }
    }

    /// The bytes of a general register.
    fn word(self) -> u64 {
        match self {
            Syntax::Att | Syntax::Arm64 => 8,
            Syntax::Arm32 => 4,
        }
    }

    /// Whether `line` returns from the function.
    pub(super) fn returns(self, line: &str) -> bool {
        let line = line.trim();
        match self {
            Syntax::Att => line.starts_with("ret"),
            Syntax::Arm64 | Syntax::Arm32 => line.starts_with("ret") || line.starts_with("bx"),
        }
    }

    /// For each result register an instruction writes, what it reads.
    pub(super) fn writes(self, line: &str) -> Vec<(Dest, Seen)> {
        let code = match self {
            Syntax::Att => line.split('#').next(),
            Syntax::Arm64 | Syntax::Arm32 => line.split("//").next().and_then(|l| l.split([';', '@']).next()),
        };
        let Some((_, operands)) = code.unwrap_or_default().trim().split_once(char::is_whitespace) else {
            return Vec::new();
        };
        let operands = Self::operands(operands);
        let read = |operand: &str| {
            let operand = operand.trim_start_matches('%');
            self.register(operand).map(|(c, _)| Seen::Reg(c)).or_else(|| self.stack(operand).map(Seen::Stack))
        };
        let dest = |operand: &str| {
            let (carrier, result) = self.register(operand.trim_start_matches('%'))?;
            Some((matches!(carrier, Carrier::Float(_)), result))
        };
        match self {
            Syntax::Att => {
                let (Some(last), Some(first)) = (operands.last(), operands.first()) else {
                    return Vec::new();
                };
                dest(last).zip(read(first)).into_iter().collect()
            }
            Syntax::Arm64 | Syntax::Arm32 => {
                let word = self.word();
                let memory = operands.iter().find_map(|o| self.stack(o));
                let dests: Vec<_> = operands.iter().map_while(|o| dest(o)).collect();
                match (memory, dests.as_slice()) {
                    (Some(at), dests) => dests
                        .iter()
                        .enumerate()
                        .map(|(k, &d)| (d, Seen::Stack(at + word * k as u64)))
                        .collect(),
                    (None, [d, ..]) => {
                        operands[1..].iter().find_map(|o| read(o)).map(|s| (*d, s)).into_iter().collect()
                    }
                    (None, []) => Vec::new(),
                }
            }
        }
    }

    /// The operands of an instruction, split at commas outside brackets.
    fn operands(text: &str) -> Vec<&str> {
        let (mut operands, mut depth, mut start) = (Vec::new(), 0, 0);
        for (i, c) in text.char_indices() {
            match c {
                '[' | '(' => depth += 1,
                ']' | ')' => depth -= 1,
                ',' if depth == 0 => {
                    operands.push(text[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        operands.push(text[start..].trim());
        operands.into_iter().filter(|o| !o.is_empty()).collect()
    }
}
