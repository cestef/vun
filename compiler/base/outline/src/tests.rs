use super::{Outline, Outlined};

struct Printer(Outline);

impl Outlined for Printer {
    fn outline(&mut self) -> &mut Outline {
        &mut self.0
    }
}

#[test]
fn children_are_indented_below_their_parent() {
    let mut p = Printer(Outline::new());
    p.node(format_args!("root"), |p| {
        p.node(format_args!("a"), |p| p.outline().line(format_args!("leaf")));
        p.outline().line(format_args!("b"));
    });
    assert_eq!(p.0.finish(), "root\n  a\n    leaf\n  b\n");
}
