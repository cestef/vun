use super::*;

fn at(line: u32, id: &str) -> At {
    At { line, id: id.to_owned() }
}

fn reported(line: u32, id: &str, col: u32, text: &str) -> Reported {
    Reported { at: at(line, id), col, text: text.to_owned() }
}

#[test]
fn reads_markers_columns_and_fragments() {
    let text =
        "x: u8 = 300; //~ int.does_not_fit\ny := x + ;\n//~^ syntax.expected_expression @9 \"found `;`\"\n";
    let expected = Expected::all(text).unwrap();
    assert_eq!(expected[0], Expected { at: at(1, "int.does_not_fit"), col: None, fragments: vec![] });
    assert_eq!(
        expected[1],
        Expected {
            at: at(2, "syntax.expected_expression"),
            col: Some(9),
            fragments: vec!["found `;`".into()]
        }
    );
    assert_eq!(Expected::all("//~^ a"), Err(Malformed::AboveFile { line: 1 }));
    assert_eq!(Expected::all("x //~ a \"open"), Err(Malformed::Fragment { line: 1 }));
}

#[test]
fn compare_reports_missing_unexpected_and_differing() {
    let expected = Expected::all("a //~ x\nb //~ y @3 \"needle\"\n").unwrap();
    let runs = [reported(2, "y", 3, "no match here"), reported(3, "z", 1, "")];
    let m = Mismatch::between(&expected, &runs);
    assert_eq!(m.missing, [at(1, "x")].into());
    assert_eq!(m.unexpected, [at(3, "z")].into());
    assert_eq!(m.differing.len(), 1);
    assert!(Mismatch::between(&expected[1..], &[reported(2, "y", 3, "a needle")]).is_empty());
}

#[test]
fn pages_hold_blocks_by_section() {
    let page = "# id\n\n## Example\n\n```vun build.vun\npackage := 1;\n```\n\n```vun\nx := 1; //~ a.b\n```\n\n## Fix\n\n```console\n$ vun check\n```\n";
    let page = super::Page::parse(page);
    let example = page.section("Example");
    assert_eq!(example.len(), 2);
    assert_eq!(
        (example[0].file.as_deref(), example[0].text.as_str()),
        (Some("build.vun"), "package := 1;\n")
    );
    assert_eq!((example[1].lang.as_str(), example[1].file.as_deref()), ("vun", None));
    assert_eq!(page.section("Fix")[0].lang, "console");
    assert!(page.section("Absent").is_empty());
}
