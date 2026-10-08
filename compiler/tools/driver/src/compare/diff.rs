//! How two runs of a program differ: the first thing that does.

use std::borrow::Cow;

use super::{Ending, Ran};

/// A stream a program writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// How much of a stream is part of what a run did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// All of it, byte by byte.
    Exact,
    /// What was written before the end, which one of two runs may have written less of: output
    /// buffered by the program is lost when it traps.
    Prefix,
    /// None: it says why the program stopped, in words each backend has of its own.
    Ignored,
}

impl Stream {
    /// How much of the stream counts, for a run that ended as `ending`.
    #[must_use]
    pub const fn policy(self, ending: Ending) -> Policy {
        match (self, ending.returned()) {
            (_, true) => Policy::Exact,
            (Self::Stdout, false) => Policy::Prefix,
            (Self::Stderr, false) => Policy::Ignored,
        }
    }
}

/// What parts two runs: how they ended, else the first line of a stream they wrote differently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Difference {
    Ending {
        reference: Ending,
        other: Ending,
    },
    /// Line `line`, counted from 1, of `stream`; `None` where the stream had already ended.
    Output {
        stream: Stream,
        line: usize,
        reference: Option<String>,
        other: Option<String>,
    },
    /// The same lines, but only one of them ends the last in a line break: whether `reference` does.
    Terminator {
        stream: Stream,
        reference: bool,
    },
}

impl Ran {
    /// Where `self` parts from `reference`; none when they ended and wrote alike.
    #[must_use]
    pub fn differs(&self, reference: &Self) -> Option<Difference> {
        let (reference_ending, ending) = (reference.ending(), self.ending());
        if reference_ending != ending {
            return Some(Difference::Ending { reference: reference_ending, other: ending });
        }
        let streams = [
            (Stream::Stdout, &reference.output.stdout, &self.output.stdout),
            (Stream::Stderr, &reference.output.stderr, &self.output.stderr),
        ];
        streams.into_iter().find_map(|(stream, reference, other)| {
            let compared = match stream.policy(ending) {
                Policy::Exact => true,
                Policy::Prefix => !(reference.starts_with(other) || other.starts_with(reference)),
                Policy::Ignored => false,
            };
            compared.then(|| Self::first_line(stream, reference, other)).flatten()
        })
    }

    fn first_line(stream: Stream, reference: &[u8], other: &[u8]) -> Option<Difference> {
        let (mut reference_lines, mut other_lines) = (Lines::of(reference), Lines::of(other));
        let text = |line: Option<&[u8]>| line.map(|l| Cow::into_owned(String::from_utf8_lossy(l)));
        for line in 1.. {
            let (r, o) = (reference_lines.next(), other_lines.next());
            if r != o {
                return Some(Difference::Output { stream, line, reference: text(r), other: text(o) });
            }
            if r.is_none() {
                break;
            }
        }
        (reference_lines.terminated != other_lines.terminated)
            .then_some(Difference::Terminator { stream, reference: reference_lines.terminated })
    }
}

/// The lines of a stream, and whether its last one ends in a line break.
struct Lines<'a> {
    parts: std::slice::Split<'a, u8, fn(&u8) -> bool>,
    terminated: bool,
}

impl<'a> Lines<'a> {
    fn of(bytes: &'a [u8]) -> Self {
        let terminated = bytes.ends_with(b"\n");
        let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        let mut parts = body.split((|b| *b == b'\n') as fn(&u8) -> bool);
        if bytes.is_empty() {
            parts.by_ref().for_each(drop);
        }
        Self { parts, terminated }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        self.parts.next()
    }
}
