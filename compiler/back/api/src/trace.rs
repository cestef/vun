//! Writing the table of the program's code (`lang.trace`, spec 07, section 3): a row per function
//! and per trap site, and a row per origin they name, with its place in source, which only code
//! generation knows. Lowering made the table's datum and recorded how `lang.trace` lays it out.

use vun_hash::Map;
use vun_intern::Symbol;
use vun_items::lang::{
    trace_Function as function, trace_Origin as origin, trace_Site as site, trace_Table as table,
    trace_Text as text,
};
use vun_lir::unit::{Data, Reloc, Unit};
use vun_lir::{Address, Conv, DataId, Fields, Linkage, Trace, TraceLayout};
use vun_origin::OriginId;
use vun_target::{Detail, Endian};

use crate::Lines;

/// A unit whose runtime reads the table of its code.
pub trait Traced {
    /// Writes the table, if the program reads it, placing the places in source of its origins
    /// as `lines` knows them.
    fn trace(&mut self, lines: &dyn Lines, endian: Endian);
}

impl Traced for Unit {
    fn trace(&mut self, lines: &dyn Lines, endian: Endian) {
        let Some(Trace { table: at, layout, detail }) = self.trace.clone() else { return };
        let mut rows = Rows {
            layout: &layout,
            endian,
            text: Vec::new(),
            texts: Map::default(),
            origins: Vec::new(),
            indices: Map::default(),
        };
        // The functions, where the table describes the calls that led to a trap.
        let calls = self.functions.iter().filter(|_| detail == Detail::Calls);
        let named: Vec<_> = calls.map(|(id, f)| (id, f.origin, f.signature.conv != Conv::Vun)).collect();
        let mut functions = rows.block(&layout.function, named.len());
        for (row, (id, at, foreign)) in named.into_iter().enumerate() {
            functions.address(row, function::at.symbol(), Address::Func(id));
            let name = rows.text(&lines.name(at).unwrap_or_default());
            functions.text(row, function::name.symbol(), name);
            functions.int(row, function::origin.symbol(), rows.origin(lines, at));
            functions.int(row, function::foreign.symbol(), foreign.into());
        }
        let mut sites = rows.block(&layout.site, self.sites.len());
        for (row, s) in self.sites.iter().enumerate() {
            sites.int(row, site::origin.symbol(), rows.origin(lines, s.at));
            let message = rows.text(&s.msg.as_ref().map(ToString::to_string).unwrap_or_default());
            sites.text(row, site::message.symbol(), message);
        }
        let mut origins = rows.block(&layout.origin, rows.origins.len());
        for (row, &(o, into)) in rows.origins.clone().iter().enumerate() {
            let name = rows.text(&lines.name(o).unwrap_or_default());
            origins.text(row, origin::name.symbol(), name);
            if let Some(into) = into {
                origins.int(row, origin::inlined.symbol(), 1);
                origins.int(row, origin::into.symbol(), into);
            }
            let Some(line) = lines.line(o) else { continue };
            let file = rows.text(&line.file.display().to_string());
            origins.text(row, origin::file.symbol(), file);
            origins.int(row, origin::line.symbol(), line.line.into());
            origins.int(row, origin::column.symbol(), line.column.into());
        }
        let source = self.data[at].origin;
        let counts = [functions.rows, sites.rows, origins.rows];
        let [functions, sites, origins] = [functions, sites, origins].map(|b| b.place(self, source));
        let text = Block::datum(self, rows.text, Vec::new(), 1, source);
        let mut header = Block::new(&layout, &layout.table, endian, 1);
        header.address(0, table::anchor.symbol(), Address::Data(at));
        let placed = [
            (table::functions, table::function_count, functions, counts[0]),
            (table::sites, table::site_count, sites, counts[1]),
            (table::origins, table::origin_count, origins, counts[2]),
        ];
        for (rows, count, data, n) in placed {
            if let Some(data) = data {
                header.address(0, rows.symbol(), Address::Data(data));
            }
            header.int(0, count.symbol(), n as u64);
        }
        if let Some(text) = text {
            header.address(0, table::text.symbol(), Address::Data(text));
        }
        let datum = &mut self.data[at];
        datum.bytes = header.bytes;
        datum.relocs = header.relocs;
    }
}

/// The table's text and origins as its rows are written: each text once, each origin once, by
/// its index.
struct Rows<'a> {
    layout: &'a TraceLayout,
    endian: Endian,
    text: Vec<u8>,
    texts: Map<String, (u64, u64)>,
    /// Each row's origin, and the row of the code it is inlined into.
    origins: Vec<(OriginId, Option<u64>)>,
    indices: Map<(OriginId, Option<u64>), u64>,
}

impl<'a> Rows<'a> {
    fn block(&self, fields: &'a Fields, rows: usize) -> Block<'a> {
        Block::new(self.layout, fields, self.endian, rows)
    }

    /// Where `s` is in the text: its offset and length.
    fn text(&mut self, s: &str) -> (u64, u64) {
        if let Some(&span) = self.texts.get(s) {
            return span;
        }
        let span = (self.text.len() as u64, s.len() as u64);
        self.text.extend_from_slice(s.as_bytes());
        self.texts.insert(s.to_owned(), span);
        span
    }

    /// The index of the row of `at`: the code it is in, with a row for each function it is
    /// inlined into, outermost first.
    fn origin(&mut self, lines: &dyn Lines, at: OriginId) -> u64 {
        let mut code = at;
        let mut into = Vec::new();
        while let Some((copied, function)) = lines.inlined(code) {
            into.push(function);
            code = copied;
        }
        let mut outer = None;
        for level in into.into_iter().chain([code]) {
            let key = (level, outer);
            let row = *self.indices.entry(key).or_insert_with(|| {
                self.origins.push(key);
                self.origins.len() as u64 - 1
            });
            outer = Some(row);
        }
        outer.expect("a row for the code itself")
    }
}

/// Rows of one struct of `lang.trace`, written into the bytes of a datum.
struct Block<'a> {
    layout: &'a TraceLayout,
    fields: &'a Fields,
    endian: Endian,
    rows: usize,
    bytes: Vec<u8>,
    relocs: Vec<Reloc>,
}

impl<'a> Block<'a> {
    fn new(layout: &'a TraceLayout, fields: &'a Fields, endian: Endian, rows: usize) -> Self {
        let bytes = vec![0; usize::try_from(fields.size).unwrap_or_default() * rows];
        Self { layout, fields, endian, rows, bytes, relocs: Vec::new() }
    }

    /// The offset of the field `name` of row `row`, and its size.
    fn at(&self, row: usize, name: Symbol) -> (u64, u64) {
        let field = self.fields.field(name).expect("`lang.trace` declares the field");
        (row as u64 * self.fields.size + field.offset, field.bytes)
    }

    fn int(&mut self, row: usize, name: Symbol, value: u64) {
        let (offset, bytes) = self.at(row, name);
        self.put(offset, bytes, value);
    }

    fn address(&mut self, row: usize, name: Symbol, target: Address) {
        let (offset, _) = self.at(row, name);
        self.relocs.push(Reloc { offset, target, addend: 0 });
    }

    /// Writes the text `span` into the field `name`, a `lang.trace.Text`.
    fn text(&mut self, row: usize, name: Symbol, (start, len): (u64, u64)) {
        let (offset, _) = self.at(row, name);
        for (part, value) in [(text::at, start), (text::len, len)] {
            let field = self.layout.text.field(part.symbol()).expect("`lang.trace.Text` declares it");
            self.put(offset + field.offset, field.bytes, value);
        }
    }

    fn put(&mut self, offset: u64, bytes: u64, value: u64) {
        let (offset, bytes) =
            (usize::try_from(offset).unwrap_or_default(), usize::try_from(bytes).unwrap_or_default());
        let place = &mut self.bytes[offset..offset + bytes];
        match self.endian {
            Endian::Little => place.copy_from_slice(&value.to_le_bytes()[..bytes]),
            Endian::Big => place.copy_from_slice(&value.to_be_bytes()[8 - bytes..]),
        }
    }

    /// The datum holding the rows, none for no rows.
    fn place(self, unit: &mut Unit, origin: OriginId) -> Option<DataId> {
        Self::datum(unit, self.bytes, self.relocs, self.fields.align, origin)
    }

    /// A read-only datum of the table holding `bytes`, none for none.
    fn datum(
        unit: &mut Unit,
        bytes: Vec<u8>,
        relocs: Vec<Reloc>,
        align: u64,
        origin: OriginId,
    ) -> Option<DataId> {
        let data =
            Data { linkage: Linkage::Internal, bytes, relocs, align, writable: false, section: None, origin };
        (!data.bytes.is_empty()).then(|| unit.data.push(data))
    }
}
