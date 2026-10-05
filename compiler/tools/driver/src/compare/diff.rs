//! How two runs of a program differ: the first thing that does.

use std::borrow::Cow;

use super::Ran;

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

/// What parts two runs: how they ended, else the first line of a stream they wrote differently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Difference {
    /// The exit codes; `None` is a signal.
    Exit { reference: Option<i32>, other: Option<i32> },
    /// Line `line`, counted from 1, of `stream`; `None` where the stream had already ended.
    Output { stream: Stream, line: usize, reference: Option<String>, other: Option<String> },
    /// The same lines, but only one of them ends the last in a line break: whether `reference` does.
    Terminator { stream: Stream, reference: bool },
}

impl Ran {
    /// Where `self` parts from `reference`; none when they ended and wrote alike.
    #[must_use]
    pub fn differs(&self, reference: &Self) -> Option<Difference> {
        let (reference_code, other_code) = (reference.output.status.code(), self.output.status.code());
        if reference_code != other_code {
            return Some(Difference::Exit { reference: reference_code, other: other_code });
        }
        let streams = [
            (Stream::Stdout, &reference.output.stdout, &self.output.stdout),
            (Stream::Stderr, &reference.output.stderr, &self.output.stderr),
        ];
        streams.into_iter().find_map(|(stream, reference, other)| Self::first_line(stream, reference, other))
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
