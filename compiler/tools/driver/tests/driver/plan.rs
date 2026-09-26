//! A package's plan (spec 12, section 2): `build` runs at compile time on the `std.build.Build`
//! `std.build.plan` lends it, and the driver reads back the actions it recorded.

use vun_build::layout::MANIFEST;
use vun_diag::errors;
use vun_hir::db::Package;
use vun_span::Span;

use crate::common::Fixture;

const PACKAGE: &str = "\
package := .{
    .name = \"blinky\",
    .id = 1,
    .version = \"0.1.0\",
    .edition = 2026,
    .include = .{ \"build.vun\" },
};
";

impl Fixture {
    /// The actions the plan of a package whose `build.vun` holds `build` records, as literals.
    fn planned(&self, build: &str) -> Option<Vec<String>> {
        let path = self.root.write(MANIFEST, format!("{PACKAGE}\n{build}"));
        let mut session = self.session();
        let file = session.load(&path).unwrap();
        let plan = session.plan(file)?;
        let package = |p| session.db.get::<Package>(&p).name;
        let span = Span::new(file, 0, 0);
        Some(plan.actions.iter().map(|a| a.literal(span, &package).unwrap().source().unwrap()).collect())
    }
}

#[test]
fn a_plan_records_each_step_with_its_records() {
    let fixture = Fixture::new("plan-steps");
    let actions = fixture
        .planned(
            "\
build := (b: &mut std.build.Build) {
    chip := b.dep(.stm32f4, .{ .variant = .f407 });
    fw := b.program(.{ .root = \"app.vun\", .target = targets.bare.cortex_m4f, .deps = .{ .stm32f4 = chip } });
    b.flash(fw, .probe(.{ .chip = \"STM32F407VGTx\" }));
    b.test_on(fw, .qemu(\"lm3s6965\"));
    b.lints(.{ .{ std.lints.pedantic, .warn } });
};
",
    )
    .expect("the plan runs");
    let target = ".{ .arch = .arm, .cpu = .cortex_m4f, .os = .bare, .abi = .eabihf, .pointer_bits = 32, \
                  .endian = .little, \
                  .object_format = .elf, .entry_symbol = .some(\"Reset\"), .backends = .{ .debug = .{ .cranelift, .c }, \
                  .release = .{ .llvm, .c } }, .linker = .lld, .c = .{ .short = .{ .bits = 16, .align = 2 }, \
                  .int = .{ .bits = 32, .align = 4 }, .long = .{ .bits = 32, .align = 4 }, \
                  .long_long = .{ .bits = 64, .align = 8 }, .pointer_align = 4, \
                  .compiler = .{ .command = \"arm-none-eabi-gcc\", \
                  .flags = .{ \"-std=c99\", \"-ffreestanding\", \"-fno-strict-aliasing\" }, \
                  .optimize = .{ \"-O2\" }, .link_flags = .{ \"-nostdlib\", \"-nostartfiles\" }, .atomics = .gnu, \
                  .frame_sizes = .stack_usage } } }";
    assert_eq!(
        actions,
        [
            ".dep(.{ .name = .stm32f4, .params = .{ .variant = .f407 } })".to_owned(),
            format!(".program(.{{ .deps = .{{ .stm32f4 = .{{ .step = .{{ .index = 0 }} }} }}, .root = \"app.vun\", .target = {target} }})"),
            ".flash(.{ .program = .{ .step = .{ .index = 1 } }, .method = .probe(.{ .chip = \"STM32F407VGTx\" }) })".to_owned(),
            ".test_on(.{ .program = .{ .step = .{ .index = 1 } }, .runner = .qemu(\"lm3s6965\") })".to_owned(),
            ".lints(.{ .{ std.lints.pedantic, .warn } })".to_owned(),
        ]
    );
}

#[test]
fn an_empty_build_plans_nothing() {
    let fixture = Fixture::new("plan-empty");
    assert_eq!(fixture.planned("build := (b: &mut std.build.Build) {};\n"), Some(Vec::new()));
}

#[test]
fn checking_a_manifest_runs_its_plan() {
    let fixture = Fixture::new("plan-check");
    let missing = fixture.root.write(MANIFEST, PACKAGE);
    assert_eq!(fixture.check(&missing), [errors::PLAN_MISSING_BUILD]);
    let trap = fixture.root.write(
        MANIFEST,
        format!(
            "{PACKAGE}\nbuild := (b: &mut std.build.Build) {{ n: u8 = 0; b.program(.{{ .n = 1 / n }}); }};\n"
        ),
    );
    assert_eq!(fixture.check(&trap), [errors::COMPTIME_TRAP]);
}
