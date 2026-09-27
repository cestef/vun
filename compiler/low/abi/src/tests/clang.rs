//! Placements checked against clang's (spec 13, section 8.5): for each case, a C function per
//! argument returns that argument, so the one or two instructions clang compiles it to read the
//! argument from where the convention passes it. Skipped where clang is not installed.

use std::process::Command;

use vun_lir::{AbiType, Conv, Ext, Scalar, Signature};

use super::{AAPCS64, DARWIN, SYSV};
use crate::{Carrier, Convention, Loc, Passing};

/// A C type an argument has.
#[derive(Clone, Copy, Debug)]
enum C {
    Char,
    Short,
    Long,
    Float,
    Double,
    Int128,
}

/// Where a function's code reads an argument from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    Reg(Carrier<u8, u8>),
    Stack(u64),
}

/// How a target's assembly is written.
#[derive(Clone, Copy)]
enum Syntax {
    /// x86-64 in AT&T syntax: the destination last, the stack above the return address.
    Att,
    /// arm64: the destination first, the stack at `sp`.
    Arm,
}

/// A target clang compiles for, and the convention the classifier follows there.
struct Target {
    triple: &'static str,
    convention: &'static Convention<u8, u8>,
    syntax: Syntax,
}

const TARGETS: [Target; 3] = [
    Target { triple: "x86_64-linux-gnu", convention: &SYSV, syntax: Syntax::Att },
    Target { triple: "aarch64-linux-gnu", convention: &AAPCS64, syntax: Syntax::Arm },
    Target { triple: "arm64-apple-macos", convention: &DARWIN, syntax: Syntax::Arm },
];

const L: C = C::Long;
/// Signatures that reach each rule: classes filling up, narrow scalars on the stack, and wide
/// integers in registers, split by the last free one, or on the stack.
const CASES: [&[C]; 5] = [
    &[L, C::Int128, L, C::Float, C::Double, C::Char, C::Short],
    &[L, L, L, L, L, L, L, L, C::Char, C::Char, C::Short],
    &[L, L, L, L, L, L, L, C::Int128, L],
    &[
        C::Double,
        C::Double,
        C::Double,
        C::Double,
        C::Double,
        C::Double,
        C::Double,
        C::Double,
        C::Float,
        C::Float,
    ],
    &[L, L, L, L, L, L, L, L, C::Char, C::Int128, C::Char],
];

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

impl C {
    fn spelling(self) -> &'static str {
        match self {
            Self::Char => "signed char",
            Self::Short => "short",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
            Self::Int128 => "__int128",
        }
    }

    fn abi(self) -> AbiType {
        match self {
            Self::Char => AbiType::Int(Scalar::I8, Ext::Sign),
            Self::Short => AbiType::Int(Scalar::I16, Ext::Sign),
            Self::Long => AbiType::Int(Scalar::I64, Ext::Sign),
            Self::Float => AbiType::Scalar(Scalar::F32),
            Self::Double => AbiType::Scalar(Scalar::F64),
            Self::Int128 => AbiType::Wide { half: Scalar::I64, ext: Ext::Sign },
        }
    }

    /// How many result registers returning it takes.
    fn halves(self) -> usize {
        if matches!(self, Self::Int128) { 2 } else { 1 }
    }

    fn float(self) -> bool {
        matches!(self, Self::Float | Self::Double)
    }
}

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
            Syntax::Arm => match number(&['x', 'w']) {
                Some(n) => Some((Carrier::Int(n), n)),
                None => number(&['d', 's', 'q', 'h', 'b', 'v']).map(|n| (Carrier::Float(n), n)),
            },
        }
    }

    /// The result register of a class at position `k`, as an argument register.
    fn result(self, float: bool, k: u8) -> Carrier<u8, u8> {
        let name = match (self, float) {
            (_, true) => return Carrier::Float(k),
            (Syntax::Att, false) => ["rax", "rdx"][usize::from(k)],
            (Syntax::Arm, false) => return Carrier::Int(k),
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
            Syntax::Arm => {
                let inside = memory.strip_prefix("[sp")?.strip_suffix(']')?;
                let offset = inside.strip_prefix(", #").unwrap_or("0");
                offset.parse().ok()
            }
        }
    }

    /// For each result register an instruction writes, what it reads.
    fn writes(self, line: &str) -> Vec<((bool, u8), Seen)> {
        let code = match self {
            Syntax::Att => line.split('#').next(),
            Syntax::Arm => line.split("//").next().and_then(|l| l.split(';').next()),
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
            Syntax::Arm => {
                let memory = operands.iter().find_map(|o| self.stack(o));
                let dests: Vec<_> = operands.iter().map_while(|o| dest(o)).collect();
                match (memory, dests.as_slice()) {
                    (Some(at), dests) => {
                        dests.iter().enumerate().map(|(k, &d)| (d, Seen::Stack(at + 8 * k as u64))).collect()
                    }
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

/// The assembly of `source` for `triple`, or `None` where clang is not installed.
///
/// # Panics
/// If clang cannot compile it.
fn assembly(triple: &str, source: &str) -> Option<String> {
    let dir = vun_fs::Scratch::create("clang").expect("a scratch directory");
    let path = dir.put("case.c", source).expect("the case written");
    let output = Command::new("clang")
        .args(["-O1", "-S", "-fno-asynchronous-unwind-tables", "-o", "-"])
        .arg(format!("--target={triple}"))
        .arg(&path)
        .output();
    let output = match output {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        other => other.expect("clang runs"),
    };
    assert!(output.status.success(), "clang for {triple}: {}", String::from_utf8_lossy(&output.stderr));
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Where the code of the function returning argument `k` of `case` reads it from.
fn seen(target: &Target, text: &str, case: &[C], k: usize) -> Vec<Seen> {
    let label = format!("f{k}:");
    let body = text.lines().skip_while(|l| !l.trim_start_matches('_').starts_with(&label)).skip(1);
    let body: Vec<&str> = body.take_while(|l| !l.trim().starts_with("ret")).collect();
    let writes: Vec<_> = body.iter().flat_map(|l| target.syntax.writes(l)).collect();
    let float = case[k].float();
    (0..case[k].halves())
        .map(|half| {
            let result = u8::try_from(half).expect("two halves");
            let written = writes.iter().find(|&&(d, _)| d == (float, result)).map(|&(_, s)| s);
            // Unwritten, the argument is in the result register already.
            written.unwrap_or_else(|| Seen::Reg(target.syntax.result(float, result)))
        })
        .collect()
}

#[test]
fn placements_agree_with_clang() {
    for target in &TARGETS {
        for case in CASES {
            let params = case.iter().map(|c| c.spelling()).enumerate().map(|(k, t)| format!("{t} a{k}"));
            let params = params.collect::<Vec<_>>().join(", ");
            let functions = case
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{} f{k}({params}) {{ return a{k}; }}\n", c.spelling()));
            let Some(text) = assembly(target.triple, &functions.collect::<String>()) else { return };
            let signature =
                Signature { conv: Conv::C, params: case.iter().map(|c| c.abi()).collect(), result: None };
            let passing = Passing::of(&signature, target.convention).unwrap();
            let mut ours = passing.params.iter().map(|loc| match *loc {
                Loc::Reg(c) => Seen::Reg(c),
                Loc::Stack { offset, .. } => Seen::Stack(offset),
                ref other => panic!("a scalar placed as {other:?}"),
            });
            for k in 0..case.len() {
                let expected: Vec<Seen> = ours.by_ref().take(case[k].halves()).collect();
                assert_eq!(
                    seen(target, &text, case, k),
                    expected,
                    "{} {case:?}, argument {k}",
                    target.triple
                );
            }
        }
    }
}
