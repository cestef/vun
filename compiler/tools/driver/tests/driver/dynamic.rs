//! Dynamic dispatch is a library (spec 02, section 3): `std.Dyn(I)` calls the functions of the
//! witness of a type through a table, as every backend runs it.

use crate::backend;

/// Two types behind one `Dyn(Shape)` (`9 + 20`), and a `&mut self` method with arguments, a
/// result and a provided default through `Dyn(Encoder)` (`5 + 1 + 2`).
const PROGRAM: &str = "\
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
} });
Square := struct { side: u64 };
impl Shape for Square {
    area := (self) -> u64 { (self.side * self.side).wrap(u64) };
}
Rect := struct { w: u64, h: u64 };
impl Shape for Rect {
    area := (self) -> u64 { (self.w * self.h).wrap(u64) };
}
total := (s: &std.Dyn(Shape)) -> u64 { s.area() };

Encoder := interface(($Self: type) -> type { struct {
    Error := enum { full };
    begin: (self: &mut Self, name: &[u8], count: u32) -> Result((), Error) = _;
    field: (self: &mut Self, name: &[u8]) -> Result((), Error) = _;
    pair := (self: &mut Self, a: &[u8], b: &[u8]) -> Result((), Error) { self.field(a)?; self.field(b) };
    total: (self: &Self) -> u32 = _;
} });
Counter := struct { n: u32 };
impl Encoder for Counter {
    begin := (self, name, count) { self.n = (self.n + count).wrap(u32); .ok(()) };
    field := (self, name) {
        if self.n > 100 { return .err(.full); }
        self.n = (self.n + 1).wrap(u32);
        .ok(())
    };
    total := (self) { self.n };
}
emit := (w: &mut std.Dyn(Encoder)) -> Result((), std.Dyn(Encoder).Error) {
    w.begin(\"point\", 5)?;
    w.field(\"x\")?;
    w.pair(\"y\", \"z\")
};

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    sq := Square{ .side = 3 };
    r := Rect{ .w = 4, .h = 5 };
    a := unsafe std.Dyn(Shape).of(Square, lang.mem.pointer_to(Square, &sq));
    b := unsafe std.Dyn(Shape).of(Rect, lang.mem.pointer_to(Rect, &r));
    mut c := Counter{ .n = 0 };
    mut d := unsafe std.Dyn(Encoder).of(Counter, lang.mem.pointer_to(Counter, &c));
    ok := if .ok(_) = emit(&mut d) { true } else { false };
    areas := (total(&a) + total(&b)).to(u32);
    std.os.Exit.of((areas + d.total() + if ok { 0 } else { 50 }).to(Int(0..=125)))
};
";

#[test]
fn a_type_is_called_through_the_table_of_its_witness() {
    assert_eq!(backend::status_across("dynamic", PROGRAM), Some(37));
}
