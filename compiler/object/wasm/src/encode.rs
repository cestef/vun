//! Writing the encoding's shapes: names, sections and subsections, types and limits.

use vun_bytes::Out;

use crate::format::{FUNC_TYPE, FuncType, Limits, Section, op};

/// Whether limits carry a maximum.
const MIN: u8 = 0;
const MIN_MAX: u8 = 1;

/// The encodings of wasm on bytes being written.
pub trait Wasm {
    /// A name: its length, then its bytes.
    fn name(&mut self, name: &[u8]);

    /// A section or subsection: its id, its length, then `payload`.
    fn subsection(&mut self, id: u8, payload: &Out);

    fn section(&mut self, section: Section, payload: &Out);

    /// A custom section named `name` holding `payload`.
    fn custom(&mut self, name: &str, payload: &Out);

    fn func_type(&mut self, ty: &FuncType);

    fn limits(&mut self, limits: Limits);

    /// A constant expression giving `value` as an `i32`: where an active segment starts, or a
    /// global's first value.
    fn active(&mut self, value: i64);

    /// A vector: its length, then each item as `item` writes it.
    fn vector<T>(&mut self, items: impl ExactSizeIterator<Item = T>, item: impl FnMut(&mut Self, T));
}

impl Wasm for Out {
    fn name(&mut self, name: &[u8]) {
        self.uleb(name.len() as u64);
        self.bytes(name);
    }

    fn subsection(&mut self, id: u8, payload: &Out) {
        self.u8(id);
        self.uleb(payload.here());
        self.bytes(&payload.0);
    }

    fn section(&mut self, section: Section, payload: &Out) {
        self.subsection(section as u8, payload);
    }

    fn custom(&mut self, name: &str, payload: &Out) {
        let mut out = Out::default();
        out.name(name.as_bytes());
        out.bytes(&payload.0);
        self.section(Section::Custom, &out);
    }

    fn func_type(&mut self, ty: &FuncType) {
        self.u8(FUNC_TYPE);
        for list in [&ty.params, &ty.results] {
            self.vector(list.iter(), |out, t| out.u8(t.code()));
        }
    }

    fn limits(&mut self, limits: Limits) {
        match limits.max {
            None => {
                self.u8(MIN);
                self.uleb(u64::from(limits.min));
            }
            Some(max) => {
                self.u8(MIN_MAX);
                self.uleb(u64::from(limits.min));
                self.uleb(u64::from(max));
            }
        }
    }

    fn active(&mut self, value: i64) {
        self.u8(op::I32_CONST);
        self.sleb(value);
        self.u8(op::END);
    }

    fn vector<T>(&mut self, items: impl ExactSizeIterator<Item = T>, mut item: impl FnMut(&mut Self, T)) {
        self.uleb(items.len() as u64);
        for it in items {
            item(self, it);
        }
    }
}
