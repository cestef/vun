//! What a plan gives the options of commands (spec 12, section 2): `b.commands`.

use vun_build::layout::MANIFEST;
use vun_command::Setting;

use crate::support::Fixture;

const PACKAGE: &str = "\
package := .{
    .name = \"tools\",
    .id = 1,
    .version = \"0.1.0\",
    .edition = 2026,
    .include = .{ \"build.vun\" },
};
";

fn setting(command: &str, option: &str, values: &[&str]) -> Setting {
    Setting {
        command: command.to_owned(),
        option: option.to_owned(),
        values: values.iter().map(|v| (*v).to_owned()).collect(),
    }
}

#[test]
fn a_plan_gives_each_command_the_values_of_its_options() {
    let fixture = Fixture::new("settings-values");
    fixture.root.write(
        MANIFEST,
        format!(
            "{PACKAGE}\n\
build := (b: &mut std.build.Build) {{
    b.commands(.{{
        .fmt = .{{ .width = 100 }},
        .fuzz = .{{ .cases = 5000, .seed = 7 }},
        .build = .{{ .release = true, .checks = .{{ .pointers, .allocations }} }},
        .run = .{{ .target = \"linux.x86_64.gnu\" }},
    }});
}};
"
        ),
    );
    let mut session = fixture.session();
    let (settings, _) = session.settings(Some(fixture.root.path())).expect("the plan runs");
    let all: Vec<&Setting> = settings.iter().collect();
    assert_eq!(
        all,
        [
            &setting("build", "checks", &["pointers", "allocations"]),
            &setting("build", "release", &["true"]),
            &setting("fmt", "width", &["100"]),
            &setting("fuzz", "cases", &["5000"]),
            &setting("fuzz", "seed", &["7"]),
            &setting("run", "target", &["linux.x86_64.gnu"]),
        ]
    );
}

#[test]
fn a_package_without_a_plan_gives_nothing() {
    let fixture = Fixture::new("settings-none");
    fixture.root.write(MANIFEST, format!("{PACKAGE}\nbuild := (b: &mut std.build.Build) {{}};\n"));
    let mut session = fixture.session();
    let (settings, _) = session.settings(Some(fixture.root.path())).expect("the plan runs");
    assert!(settings.is_empty());
}
