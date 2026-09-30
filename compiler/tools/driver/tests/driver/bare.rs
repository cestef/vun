//! A bare-metal program on a board's runtime (spec 08, section 2): its plan places its sections in
//! the board's memory, its runtime's vector table and reset handler start it with its variables
//! laid out, and it runs where the machine's toolchain runs programs of the target: qemu's STM32F405
//! board, the program ending through semihosting.

use vun_build::layout::MANIFEST;
use vun_harness::repo_root;
use vun_items::targets;
use vun_origin::Resolve;
use vun_sema::target::Choice;
use vun_span::SourceMap;

use crate::common::Fixture;

/// The board's package, beside the libraries.
const BOARD: &str = "lib/boards/stm32f4";

const BUILD: &str = "\
build := (b: &mut std.build.Build) {
    chip := b.dep(.stm32f4, .{});
    b.program(.{
        .root = \"fw.vun\",
        .target = targets.bare.thumbv7em.eabihf,
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

/// Blinks the board's LED at each wait of its timer, run by `std.task.run` (spec 06, section 4),
/// and ends after three: 42.
const BLINKY: &str = "\
start := stm32f4.start(.{
    .main = main,
    .trap = stm32f4.halt,
    .interrupts = .{ .tim2 = stm32f4.time.on_tim2 },
});

blink := (led: &mut stm32f4.gpio.Output, timer: &stm32f4.time.Timer) -> std.task.Async(never) {
    mut waited: u32 = 0;
    for {
        led.toggle();
        yield ..timer.after(std.time.Duration.millis(2));
        waited +%= 1;
        if waited == 3 { stm32f4.semihost.exit(42); }
    }
};

main := (p: stm32f4.Peripherals) -> never {
    mut led := p.gpioa.pin(5).output();
    mut timer := stm32f4.time.Timer.init(p.tim2);
    mut a := blink(&mut led, &timer);
    std.task.run(.{ &mut a })
};
";

/// The status `firmware` ends with on the board, built by a plan on the board's layout; `None`
/// where the machine runs no program of the target.
fn status(name: &str, firmware: &str) -> Option<i32> {
    let fixture = Fixture::new(name);
    let mut manifest = Fixture::manifest("fw", 3, "0.1.0");
    manifest.deps.push(Fixture::local("stm32f4", &repo_root().join(BOARD).display().to_string()));
    let path = fixture.root.write(MANIFEST, format!("{}\n{BUILD}", manifest.source().unwrap()));
    let firmware = fixture.root.write("fw.vun", firmware);
    let mut board = fixture.session();
    board.choice = Choice::of(targets::bare_thumbv7em::eabihf);
    let file = board.load(&firmware).unwrap();
    let _ = board.lir(file);
    if board.runner().is_empty() {
        return None;
    }
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let programs = session.carry_out(file).unwrap_or_else(|_| {
        let reports = session.diag.sorted(session.origins);
        let at = |d: &vun_diag::Diagnostic| {
            d.primary.as_ref().map(|l| session.sources.locate(session.source_span(l.origin)).to_string())
        };
        let notes =
            |d: &vun_diag::Diagnostic| d.children.iter().map(|c| c.msg.to_string()).collect::<Vec<_>>();
        panic!("{:#?}", reports.iter().map(|d| (d.message.to_string(), at(d), notes(d))).collect::<Vec<_>>())
    });
    let [program] = programs.as_slice() else { panic!("one program is built: {programs:?}") };
    board.command(program).status().unwrap().code()
}

#[test]
fn a_board_program_lays_its_memory_out_at_reset_and_runs_on_the_board() {
    if let Some(status) = status("bare-board", FIRMWARE) {
        assert_eq!(status, 42);
    }
}

#[test]
fn a_board_program_waits_on_its_timer_through_the_executor() {
    if let Some(status) = status("bare-blinky", BLINKY) {
        assert_eq!(status, 42);
    }
}
