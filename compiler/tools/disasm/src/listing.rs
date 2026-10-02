//! The listing of a file's code: each executable section, instruction by instruction, with the
//! labels its symbols give.

use std::collections::BTreeMap;
use std::fmt;

use crate::code::{Code, Error, Part, Symbol};

/// A line of a listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// The start of a section's code.
    Section { name: Vec<u8>, address: u64 },
    /// A symbol naming the address that follows.
    Label { address: u64, name: Vec<u8> },
    /// An instruction, with the bytes it is made of and the place it refers to by its position, if
    /// it does.
    Inst { address: u64, bytes: Vec<u8>, text: String, target: Option<Target> },
    /// A byte that starts no instruction.
    Data { address: u64, byte: u8 },
}

/// Where an instruction refers to by its position, a direct branch's target or an operand relative
/// to the instruction pointer: an address, and the symbol holding it with the distance past the
/// symbol's start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub address: u64,
    pub symbol: Option<(Vec<u8>, u64)>,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.address)?;
        match &self.symbol {
            Some((name, 0)) => write!(f, " <{}>", String::from_utf8_lossy(name)),
            Some((name, past)) => write!(f, " <{}+{past:#x}>", String::from_utf8_lossy(name)),
            None => Ok(()),
        }
    }
}

/// The bytes a line leaves room for before the instruction's text; a longer instruction pushes its
/// text right.
const BYTES_COLUMN: usize = 24;

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Section { name, address } => {
                write!(f, "section {} at {address:#x}", String::from_utf8_lossy(name))
            }
            Self::Label { address, name } => {
                write!(f, "{address:016x} <{}>:", String::from_utf8_lossy(name))
            }
            Self::Inst { address, bytes, text, target } => {
                let bytes: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                write!(f, "{address:>8x}:  {:<BYTES_COLUMN$}  {text}", bytes.join(" "))?;
                target.as_ref().map_or(Ok(()), |target| write!(f, "  ; {target}"))
            }
            Self::Data { address, byte } => {
                write!(f, "{address:>8x}:  {:<BYTES_COLUMN$}  .byte {byte:#04x}", format!("{byte:02x}"))
            }
        }
    }
}

/// The listing of `code`, or of the code `only` labels: the instructions of each section from its
/// start, or from the symbol `only` names to its end (the next label where it has no size).
///
/// # Errors
/// Where no code is labelled `only`.
pub fn listing(code: &Code, only: Option<&[u8]>) -> Result<Vec<Line>, Error> {
    // Where a branch goes is named only where addresses are those of a loaded program.
    let index = code.linked.then(|| Index::of(&code.symbols));
    // The symbols each part holds, found once: a program has thousands of both.
    let mut placed: Vec<Vec<&Symbol>> = vec![Vec::new(); code.parts.len()];
    for symbol in &code.symbols {
        if let Some(held) = symbol.part.and_then(|number| placed.get_mut(number)) {
            held.push(symbol);
        }
    }
    let mut lines = Vec::new();
    for (part, held) in code.parts.iter().zip(&placed) {
        let labels = Labels::of(part, held);
        let length = part.bytes.len();
        let (start, end) = match only {
            None => (0, length),
            Some(name) => match labels.range_of(name, length) {
                Some(range) => range,
                None => continue,
            },
        };
        lines.push(Line::Section { name: part.name.clone(), address: part.address });
        let mut at = start;
        while at < end {
            for name in labels.at(at) {
                lines.push(Line::Label { address: part.address + at as u64, name: name.to_vec() });
            }
            let address = part.address + at as u64;
            // An instruction ends where the next label begins, so that padding between
            // functions never swallows the start of the next one.
            let rest = &part.bytes[at..labels.next_after(at).map_or(end, |next| next.min(end))];
            if let Some(decoded) = code.isa.decode(rest) {
                let len = decoded.len.get().min(rest.len());
                let target = decoded.target.and_then(|distance| address.checked_add_signed(distance));
                let target = target.map(|address| Target {
                    address,
                    symbol: index.as_ref().and_then(|index| index.holding(address)),
                });
                lines.push(Line::Inst { address, bytes: rest[..len].to_vec(), text: decoded.text, target });
                at += len;
            } else {
                lines.push(Line::Data { address, byte: rest[0] });
                at += 1;
            }
        }
    }
    match (only, lines.is_empty()) {
        (Some(name), true) => Err(Error::Missing(name.to_vec())),
        _ => Ok(lines),
    }
}

/// The names that label code in a section, by their offset in it.
struct Labels<'a> {
    by_offset: BTreeMap<u64, Vec<&'a [u8]>>,
    /// The size each name gives, 0 where it gives none.
    sizes: BTreeMap<&'a [u8], u64>,
}

impl<'a> Labels<'a> {
    /// The symbols `held` by the part `part`.
    fn of(part: &Part, held: &[&'a Symbol]) -> Self {
        let mut labels = Self { by_offset: BTreeMap::new(), sizes: BTreeMap::new() };
        for &symbol in held {
            let Some(offset) = symbol.address.checked_sub(part.address) else { continue };
            labels.by_offset.entry(offset).or_default().push(&symbol.name);
            labels.sizes.entry(&symbol.name).or_insert(symbol.size);
        }
        labels
    }

    /// The names at `offset`.
    fn at(&self, offset: usize) -> impl Iterator<Item = &'a [u8]> + '_ {
        self.by_offset.get(&(offset as u64)).into_iter().flatten().copied()
    }

    /// The offset of the first label past `offset`.
    fn next_after(&self, offset: usize) -> Option<usize> {
        let (&next, _) = self.by_offset.range(offset as u64 + 1..).next()?;
        usize::try_from(next).ok()
    }

    /// The offsets of the code `name` labels: from the label to its size on, or to the next label.
    fn range_of(&self, name: &[u8], length: usize) -> Option<(usize, usize)> {
        let (&start, _) = self.by_offset.iter().find(|(_, names)| names.contains(&name))?;
        let size = self.sizes.get(name).copied().unwrap_or(0);
        let end = if size > 0 {
            start.saturating_add(size)
        } else {
            self.by_offset.range(start + 1..).next().map_or(length as u64, |(&next, _)| next)
        };
        Some((usize::try_from(start).ok()?, usize::try_from(end).ok()?.min(length)))
    }
}

/// Every symbol that names a place, by its address: what names where a branch goes.
struct Index<'a> {
    by_address: BTreeMap<u64, &'a Symbol>,
}

impl<'a> Index<'a> {
    fn of(symbols: &'a [Symbol]) -> Self {
        let mut by_address = BTreeMap::new();
        for symbol in symbols {
            by_address.entry(symbol.address).or_insert(symbol);
        }
        Self { by_address }
    }

    /// The symbol holding `address`, with the distance past its start: the last one at or before
    /// it, when `address` is its start or within its size.
    fn holding(&self, address: u64) -> Option<(Vec<u8>, u64)> {
        let (&start, symbol) = self.by_address.range(..=address).next_back()?;
        let past = address - start;
        (past == 0 || past < symbol.size).then(|| (symbol.name.clone(), past))
    }
}
