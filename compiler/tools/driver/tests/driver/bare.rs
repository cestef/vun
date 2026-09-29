//! A bare-metal program on a board's runtime (spec 08, section 2): its plan places its sections in
//! the board's memory, its runtime's vector table and reset handler start it with its variables
//! laid out, and it runs where the machine's toolchain runs programs of the target: qemu's STM32F405
//! board, the program ending through semihosting.

use vun_build::layout::MANIFEST;
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_sema::target::Choice;

use crate::common::Fixture;

/// The board's package, beside the libraries.
const BOARD: &str = "lib/boards/stm32f4";

const BUILD: &str = "\
build := (b: &mut std.build.Build) {
    chip := b.dep(.stm32f4, .{});
    b.program(.{
        .root = \"fw.vun\",
        .target = targets.bare.cortex_m4f,
        .deps = &[.{ .name = .stm32f4, .dep = chip }],
        .memory = stm32f4.layout.memory,
        .sections = stm32f4.layout.sections,
    });
};
";

/// Ends with the sum of a variable loaded from flash and one zeroed at reset, each read after
/// the reset handler laid them out: 42.
const FIRMWARE: &str = "\
start := stm32f4.start(.{ .main = main, .trap = stm32f4.halt, .interrupts = .{} });

mut loaded: u32 = 40;
mut zeroed: u32 = 0;

main := (p: stm32f4.Peripherals) -> never {
    unsafe { zeroed +%= 2; }
    stm32f4.semihost.exit(unsafe { (loaded + zeroed).wrap(u32) })
};
";

#[test]
fn a_board_program_lays_its_memory_out_at_reset_and_runs_on_the_board() {
    let fixture = Fixture::new("bare-board");
    let mut manifest = Fixture::manifest("fw", 3, "0.1.0");
    manifest.deps.push(Fixture::local("stm32f4", &repo_root().join(BOARD).display().to_string()));
    let path = fixture.root.write(MANIFEST, format!("{}\n{BUILD}", manifest.source().unwrap()));
    let firmware = fixture.root.write("fw.vun", FIRMWARE);
    // Where the machine runs no program of the target, there is nothing to run it on.
    let mut board = fixture.session();
    board.choice = Choice::Path(["bare", "cortex_m4f"].map(Symbol::intern).into());
    let file = board.load(&firmware).unwrap();
    let _ = board.lir(file);
    if board.runner.is_empty() {
        return;
    }
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let programs = session.carry_out(file).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let [program] = programs.as_slice() else { panic!("one program is built: {programs:?}") };
    assert_eq!(board.command(program).status().unwrap().code(), Some(42));
}
