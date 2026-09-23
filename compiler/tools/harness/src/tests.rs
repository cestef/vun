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
    let expected = expectations(text).unwrap();
    assert_eq!(expected[0], Expected { at: at(1, "int.does_not_fit"), col: None, fragments: vec![] });
    assert_eq!(
        expected[1],
        Expected {
            at: at(2, "syntax.expected_expression"),
            col: Some(9),
            fragments: vec!["found `;`".into()]
        }
    );
    assert_eq!(expectations("//~^ a"), Err(Malformed::AboveFile { line: 1 }));
    assert_eq!(expectations("x //~ a \"open"), Err(Malformed::Fragment { line: 1 }));
}

#[test]
fn compare_reports_missing_unexpected_and_differing() {
    let expected = expectations("a //~ x\nb //~ y @3 \"needle\"\n").unwrap();
    let runs = [reported(2, "y", 3, "no match here"), reported(3, "z", 1, "")];
    let m = compare(&expected, &runs);
    assert_eq!(m.missing, [at(1, "x")].into());
    assert_eq!(m.unexpected, [at(3, "z")].into());
    assert_eq!(m.differing.len(), 1);
    assert!(compare(&expected[1..], &[reported(2, "y", 3, "a needle")]).is_empty());
}
