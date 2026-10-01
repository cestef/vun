use vun_diag::{DiagCtxt, DiagCx};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use crate::Layout;

/// `text` formatted; it must parse cleanly.
fn formatted(text: &str) -> String {
    let file = SourceFiles::new().add("input.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    assert_eq!(diag.error_count(), 0, "{text:?} does not parse");
    let once = Layout::new(&tokens, &cst, text).to_string();
    assert_ne!(once, "", "{text:?} formats to nothing");
    once
}

#[track_caller]
fn check(input: &str, expected: &str) {
    let once = formatted(input);
    assert_eq!(once, expected);
    assert_eq!(formatted(&once), once, "formatting twice changes {once:?}");
}

#[test]
fn operators_and_separators_are_spaced_by_the_tables() {
    check("x:=a+b*c;\n", "x := a + b * c;\n");
    check("y : u8 = - x ;\n", "y: u8 = -x;\n");
    check("r := 0 .. n ;\n", "r := 0..n;\n");
    check("t := ( a , b ) ;\n", "t := (a, b);\n");
    check("s := .{ .x=1 , .y=2 } ;\n", "s := .{ .x = 1, .y = 2 };\n");
    check("e := .{};\n", "e := .{};\n");
}

#[test]
fn applications_open_right_after_what_they_apply_to() {
    check("v := f ( a ) [ 0 ] . b .* ;\n", "v := f(a)[0].b.*;\n");
    check("p := Point{.x = 1};\n", "p := Point{ .x = 1 };\n");
    check("a : [ 4 ] u8 = x ;\n", "a: [4]u8 = x;\n");
    check("m := & mut x ;\n", "m := &mut x;\n");
    check("..std .{ fmt };\n", "..std.{ fmt };\n");
}

#[test]
fn open_ranges_stay_apart_from_what_follows() {
    let input = "f := (n: u8) -> u8 {\n    match n { 0..9 => 1, 10.. => 2 }\n};\n";
    check(input, input);
}

#[test]
fn lines_are_indented_by_groups_entries_and_continuations() {
    check(
        "f := () {\nif x {\n        y;\n  }\n    else { z; }\n};\n",
        "f := () {\n    if x {\n        y;\n    }\n        else { z; }\n};\n",
    );
    check("x := a\n  and b\n        and c;\n", "x := a\n    and b\n    and c;\n");
    check("c := f(\na,\n     b,\n);\n", "c := f(\n    a,\n    b,\n);\n");
}

#[test]
fn docs_and_attributes_mark_entries() {
    let input = "/// A test.\n#[test]\nt := () {\n    #[test]\n    x := 1;\n};\n";
    check(input, input);
}

#[test]
fn comments_keep_their_place() {
    check("x := 1;   // one\n", "x := 1; // one\n");
    check(
        "f := () {\n// inside\n    x;\n      // last\n};\n",
        "f := () {\n    // inside\n    x;\n    // last\n};\n",
    );
    check("// top\n\n\n\nx := 1;\n// end\n", "// top\n\nx := 1;\n// end\n");
}

#[test]
fn blank_lines_are_kept_once_and_never_inside_a_group_edge() {
    check("\n\nx := 1;\n\n\n\ny := 2;\n\n", "x := 1;\n\ny := 2;\n");
    check("f := () {\n\n    x;\n\n}; \n", "f := () {\n    x;\n};\n");
}

#[test]
fn raw_string_lines_keep_one_indent() {
    let input = "text :=\n    \\\\first\n    \\\\second\n;\n";
    check(input, input);
    check("text :=\n  \\\\first\n        \\\\second\n;\n", input);
}

#[test]
fn line_breaks_are_normalized_to_newlines() {
    check("x := 1;\r\ny := 2;\r\n", "x := 1;\ny := 2;\n");
}

#[test]
fn a_block_of_instructions_is_written_as_calls() {
    check(
        "f := () {\n    unsafe lang.arch.asm( {\n top := here( ) ;\n lea( out(r) , mem( in(p) . at(rsi) , 3 ) );\n b( ne , top );\n } );\n};\n",
        "f := () {\n    unsafe lang.arch.asm({\n        top := here();\n        lea(out(r), mem(in(p).at(rsi), 3));\n        b(ne, top);\n    });\n};\n",
    );
}

#[test]
fn a_group_on_a_line_past_the_width_is_broken_one_entry_a_line() {
    let long = "a".repeat(40);
    let call = format!("x := f({long}, {long}, {long});\n");
    let broken = format!("x := f(\n    {long},\n    {long},\n    {long}\n);\n");
    check(&call, &broken);
    // A line that fits is left alone.
    check("x := f(a, b);\n", "x := f(a, b);\n");
}

#[test]
fn the_outermost_group_is_broken_first_and_the_inner_ones_only_when_they_still_do_not_fit() {
    let long = "b".repeat(48);
    let fits = format!("x := f(g({long}, {long}), h);\n");
    check(&fits, &format!("x := f(\n    g({long}, {long}),\n    h\n);\n"));
    let wide = "c".repeat(60);
    let nested = format!("x := f(g({wide}, {wide}), h);\n");
    check(&nested, &format!("x := f(\n    g(\n        {wide},\n        {wide}\n    ),\n    h\n);\n"));
}

#[test]
fn a_line_with_nothing_to_break_stays_as_it_is() {
    let name = "d".repeat(130);
    check(&format!("{name} := 1;\n"), &format!("{name} := 1;\n"));
}

#[test]
fn a_call_of_one_group_keeps_its_opening_and_closing_on_the_lines_of_the_group() {
    let long = "e".repeat(50);
    let call = format!("x := f(.{{ .a = {long}, .b = {long} }});\n");
    check(&call, &format!("x := f(.{{\n    .a = {long},\n    .b = {long}\n}});\n"));
}

#[test]
fn a_line_without_a_group_to_break_is_continued_before_an_infix_operator() {
    let long = "g".repeat(45);
    let sum = format!("x := {long} + {long} + {long};\n");
    check(&sum, &format!("x := {long} + {long}\n    + {long};\n"));
}
