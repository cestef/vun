use vun_data::Value;
use vun_diag::{Diagnostic, Fix, Msg, Safety, TraceFrame, errors};
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use super::*;

const TEXT: &str = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
const EXPR: &str = "p.x.abs() + p.y.abs()";

struct Run {
    files: SourceFiles,
    origins: OriginTable,
}

impl Run {
    fn new() -> Self {
        Self { files: SourceFiles::new(), origins: OriginTable::new() }
    }

    fn span(&self, path: &str, needle: &str) -> Span {
        let file = self.files.add(path, TEXT);
        let lo = u32::try_from(TEXT.find(needle).unwrap()).unwrap();
        Span::new(file, lo, lo + u32::try_from(needle.len()).unwrap())
    }

    fn report(&self, sarif: &mut Sarif, d: &Diagnostic) {
        sarif.emit(d, &self.origins, &self.files);
    }
}

fn written(sarif: &Sarif) -> Value<'static> {
    let mut out = Vec::new();
    sarif.write(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1);
    vun_data::json::parse(&text).unwrap().into_owned()
}

#[test]
fn a_run_is_one_log_with_a_rule_for_each_id_and_a_result_for_each_diagnostic() {
    let run = Run::new();
    let at = run.origins.source(run.span("src/geom.vun", EXPR));
    let mut sarif = Sarif::default();
    let does_not_fit =
        Diagnostic::error(errors::INT_DOES_NOT_FIT, Msg::new().text("does not fit ").ty("u32"));
    run.report(&mut sarif, &does_not_fit.clone().at(at, None));
    run.report(&mut sarif, &does_not_fit.at(at, None));

    let log = written(&sarif);
    assert_eq!((log["version"].as_str(), log["$schema"].as_str()), (Some(VERSION), Some(SCHEMA)));
    let driver = &log["runs"][0]["tool"]["driver"];
    assert_eq!(driver["name"].as_str(), Some("vun"));
    assert_eq!(driver["rules"].as_array().map(<[_]>::len), Some(1));
    assert_eq!(driver["rules"][0]["id"].as_str(), Some("int.does_not_fit"));
    let results = &log["runs"][0]["results"];
    assert_eq!(results.as_array().map(<[_]>::len), Some(2));
    assert_eq!(results[1]["ruleIndex"].as_u64(), Some(0));
    assert_eq!(results[1]["ruleId"].as_str(), Some("int.does_not_fit"));
    assert_eq!(results[1]["level"].as_str(), Some("error"));
    assert_eq!(results[1]["message"]["text"].as_str(), Some("does not fit `u32`"));
}

#[test]
fn a_location_names_its_artifact_by_uri_and_its_region_by_lines_and_columns() {
    let run = Run::new();
    let relative = run.origins.source(run.span("src/geom.vun", EXPR));
    let absolute = run.origins.source(run.span("/work/a b/geom.vun", EXPR));
    let mut sarif = Sarif::default();
    let d = Diagnostic::warning(errors::INT_DOES_NOT_FIT, "wide")
        .at(relative, None)
        .label(absolute, Some("here".into()));
    run.report(&mut sarif, &d);

    let result = &written(&sarif)["runs"][0]["results"][0];
    assert_eq!(result["level"].as_str(), Some("warning"));
    let physical = &result["locations"][0]["physicalLocation"];
    assert_eq!(physical["artifactLocation"]["uri"].as_str(), Some("src/geom.vun"));
    assert_eq!(physical["artifactLocation"]["uriBaseId"].as_str(), Some("%SRCROOT%"));
    let region = &physical["region"];
    let corners = ["startLine", "startColumn", "endLine", "endColumn"].map(|k| region[k].as_u64());
    assert_eq!(corners, [Some(2), Some(5), Some(2), Some(26)]);
    let related = &result["relatedLocations"][0];
    assert_eq!(
        related["physicalLocation"]["artifactLocation"]["uri"].as_str(),
        Some("file:///work/a%20b/geom.vun")
    );
    assert_eq!(related["physicalLocation"]["artifactLocation"]["uriBaseId"], Value::Null);
    assert_eq!((related["id"].as_u64(), related["message"]["text"].as_str()), (Some(0), Some("here")));
}

#[test]
fn fixes_children_and_the_trace_have_the_places_sarif_gives_them() {
    let run = Run::new();
    let span = run.span("src/geom.vun", EXPR);
    let at = run.origins.source(span);
    let mut d = Diagnostic::error(errors::INT_DOES_NOT_FIT, "wide")
        .at(at, None)
        .help("widen the result")
        .fix(Fix::replace("write u64", Safety::Safe, span, "u64"));
    d.trace.push(TraceFrame { msg: "reached through this call".into(), origin: at });
    let mut sarif = Sarif::default();
    run.report(&mut sarif, &d);

    let result = &written(&sarif)["runs"][0]["results"][0];
    let change = &result["fixes"][0]["artifactChanges"][0];
    assert_eq!(change["artifactLocation"]["uri"].as_str(), Some("src/geom.vun"));
    assert_eq!(change["replacements"][0]["insertedContent"]["text"].as_str(), Some("u64"));
    assert_eq!(change["replacements"][0]["deletedRegion"]["startLine"].as_u64(), Some(2));
    assert_eq!(result["fixes"][0]["properties"]["safety"].as_str(), Some("safe"));
    assert_eq!(result["properties"]["children"][0]["kind"].as_str(), Some("help"));
    assert_eq!(result["relatedLocations"][0]["message"]["text"].as_str(), Some("reached through this call"));
}

#[test]
fn a_run_with_no_diagnostics_is_a_log_with_no_results() {
    let log = written(&Sarif::default());
    assert_eq!(log["runs"][0]["results"].as_array().map(<[_]>::len), Some(0));
}
