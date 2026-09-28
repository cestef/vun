//! Placements checked against clang's (spec 13, section 8.5): for each case, a C function per
//! argument returns that argument, so the one or two instructions clang compiles it to read the
//! argument from where the convention passes it. Skipped where clang is not installed.

mod syntax;

use std::process::Command;

use vun_lir::{AbiType, Conv, Ext, Scalar, Signature};

use super::aapcs::{SOFT, VFP};
use super::{AAPCS64, DARWIN, SYSV};
use crate::{Carrier, Convention, Loc, Passing, Ret, Rules};
use syntax::{Dest, Syntax};

/// A C type an argument has.
#[derive(Clone, Copy, Debug)]
enum C {
    Char,
    Short,
    Int,
    LongLong,
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

/// A target clang compiles for, and the convention the classifier follows there.
struct Target {
    triple: &'static str,
    flags: &'static [&'static str],
    convention: &'static Convention<u8, u8>,
    syntax: Syntax,
    cases: &'static [&'static [C]],
}

const L: C = C::LongLong;
const D: C = C::Double;
/// Signatures that reach each rule on 64-bit machines: classes filling up, narrow scalars on the
/// stack, and wide integers in registers, split by the last free one, or on the stack.
const CASES_64: &[&[C]] = &[
    &[L, C::Int128, L, C::Float, D, C::Char, C::Short],
    &[L, L, L, L, L, L, L, L, C::Char, C::Char, C::Short],
    &[L, L, L, L, L, L, L, C::Int128, L],
    &[D, D, D, D, D, D, D, D, C::Float, C::Float],
    &[L, L, L, L, L, L, L, L, C::Char, C::Int128, C::Char],
];
/// On 32-bit Arm: floats backfilling, a double closing the VFP registers on the stack, pairs of
/// words at even registers or aligned on the stack, closing the rest.
const CASES_32: &[&[C]] = &[
    &[C::Int, C::Char, C::Short, L, C::Float, D, C::Char, C::Short],
    &[C::Float, D, C::Float, D, C::Float, C::Int, L, C::Int],
    &[D, D, D, D, D, D, D, C::Float, D, C::Float],
    &[C::Int, L, C::Int, C::Int, L, C::Short],
];

const TARGETS: [Target; 5] = [
    Target {
        triple: "x86_64-linux-gnu",
        flags: &[],
        convention: &SYSV,
        syntax: Syntax::Att,
        cases: CASES_64,
    },
    Target {
        triple: "aarch64-linux-gnu",
        flags: &[],
        convention: &AAPCS64,
        syntax: Syntax::Arm64,
        cases: CASES_64,
    },
    Target {
        triple: "arm64-apple-macos",
        flags: &[],
        convention: &DARWIN,
        syntax: Syntax::Arm64,
        cases: CASES_64,
    },
    Target {
        triple: "thumbv7em-none-eabihf",
        flags: &["-mfpu=fpv5-d16"],
        convention: &VFP,
        syntax: Syntax::Arm32,
        cases: CASES_32,
    },
    Target {
        triple: "thumbv7m-none-eabi",
        flags: &[],
        convention: &SOFT,
        syntax: Syntax::Arm32,
        cases: CASES_32,
    },
];

impl C {
    fn spelling(self) -> &'static str {
        match self {
            Self::Char => "signed char",
            Self::Short => "short",
            Self::Int => "int",
            Self::LongLong => "long long",
            Self::Float => "float",
            Self::Double => "double",
            Self::Int128 => "__int128",
        }
    }

    /// The type passing it under `rules`, as legalization gives it: a scalar wider than a
    /// register as its halves.
    fn abi(self, rules: &Rules) -> AbiType {
        let scalar = match self {
            Self::Char => Scalar::I8,
            Self::Short => Scalar::I16,
            Self::Int => Scalar::I32,
            Self::LongLong => Scalar::I64,
            Self::Float => Scalar::F32,
            Self::Double => Scalar::F64,
            Self::Int128 => Scalar::I128,
        };
        let half = Scalar::int(scalar.bits(0) / 2).filter(|_| !rules.fits(scalar));
        match half {
            Some(half) => AbiType::Wide { limb: half, limbs: 2, ext: Ext::Sign },
            None if scalar.is_float() => AbiType::Scalar(scalar),
            None => AbiType::Int(scalar, Ext::Sign),
        }
    }
}

/// The result registers of a function returning `ty`, as the classifier gives them.
fn results(ty: AbiType, convention: &Convention<u8, u8>) -> Vec<Dest> {
    let signature = Signature { conv: Conv::C, params: Vec::new(), result: Some(ty), variadic: None };
    let dest = |c: Carrier<u8, u8>| match c {
        Carrier::Int(k) => (false, k),
        Carrier::Float(k) => (true, k),
    };
    match Passing::of(&signature, convention).unwrap().ret {
        Ret::Reg(c) => vec![dest(c)],
        Ret::Split(parts) => parts.iter().map(|p| dest(p.reg)).collect(),
        other => panic!("a scalar returned as {other:?}"),
    }
}

/// The assembly of `source` for `target`, or `None` where clang is not installed.
///
/// # Panics
/// If clang cannot compile it.
fn assembly(target: &Target, source: &str) -> Option<String> {
    let dir = vun_fs::Scratch::create("clang").expect("a scratch directory");
    let path = dir.put("case.c", source).expect("the case written");
    let output = Command::new("clang")
        .args(["-O1", "-S", "-fno-asynchronous-unwind-tables", "-o", "-"])
        .arg(format!("--target={}", target.triple))
        .args(target.flags)
        .arg(&path)
        .output();
    let output = match output {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        other => other.expect("clang runs"),
    };
    let triple = target.triple;
    assert!(output.status.success(), "clang for {triple}: {}", String::from_utf8_lossy(&output.stderr));
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Where the code of function `f{k}` reads what it writes to each of `dests`.
fn seen(syntax: Syntax, text: &str, k: usize, dests: &[Dest]) -> Vec<Seen> {
    let label = format!("f{k}:");
    let body = text.lines().skip_while(|l| !l.trim_start_matches('_').starts_with(&label)).skip(1);
    let body: Vec<&str> = body.take_while(|l| !syntax.returns(l)).collect();
    let writes: Vec<_> = body.iter().flat_map(|l| syntax.writes(l)).collect();
    let seen = |&dest: &Dest| {
        let written = writes.iter().find(|&&(d, _)| d == dest).map(|&(_, s)| s);
        // Unwritten, the argument is in the result register already.
        written.unwrap_or_else(|| Seen::Reg(syntax.result(dest)))
    };
    dests.iter().map(seen).collect()
}

#[test]
fn placements_agree_with_clang() {
    for target in &TARGETS {
        let rules = target.convention.rules;
        for &case in target.cases {
            let params = case.iter().map(|c| c.spelling()).enumerate().map(|(k, t)| format!("{t} a{k}"));
            let params = params.collect::<Vec<_>>().join(", ");
            let functions = case
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{} f{k}({params}) {{ return a{k}; }}\n", c.spelling()));
            let Some(text) = assembly(target, &functions.collect::<String>()) else { return };
            let abis: Vec<AbiType> = case.iter().map(|c| c.abi(rules)).collect();
            let signature = Signature { conv: Conv::C, params: abis.clone(), result: None, variadic: None };
            let passing = Passing::of(&signature, target.convention).unwrap();
            let mut ours = passing.params.iter().map(|loc| match *loc {
                Loc::Reg(c) => Seen::Reg(c),
                Loc::Stack { offset, .. } => Seen::Stack(offset),
                ref other => panic!("a scalar placed as {other:?}"),
            });
            for (k, abi) in abis.into_iter().enumerate() {
                let expected: Vec<Seen> = ours.by_ref().take(abi.values().count()).collect();
                let dests = results(abi, target.convention);
                let triple = target.triple;
                assert_eq!(
                    seen(target.syntax, &text, k, &dests),
                    expected,
                    "{triple} {case:?}, argument {k}"
                );
            }
        }
    }
}
