//! The RP2350 board (`lib/boards/rp2350`): a project from its template builds with every
//! peripheral the board offers, not only the ones the template reaches, and the UF2 file it
//! writes places the program where the boot ROM looks for it.

use vun_build::layout::MANIFEST;

use crate::support::Fixture;

/// The family of the RP2350's Arm images, which the template's flash step names.
const FAMILY: u32 = 0xe48b_ff59;
/// Where the boot ROM expects flash images to start.
const FLASH: u32 = 0x1000_0000;

/// Uses the pins, the UART, the timer through the executor and the interrupt controller.
const BOARD: &str = "\
start := rp2350.start(.{
    .main = main,
    .interrupts = .{ .external = &[.{ .irq = rp2350.irq.timer0_0, .handler = rp2350.time.on_alarm0 }] },
});

blink := (led: &mut rp2350.gpio.Output, timer: &rp2350.time.Timer) -> std.task.Async(never) {
    for {
        led.toggle();
        yield ..timer.after(.millis(500));
    }
};

main := (p: rp2350.Peripherals) -> never {
    mut led := p.gpio.pin(15).drive(.ma8).output();
    input := p.gpio.pin(14).input(.up);
    mut timer := rp2350.time.Timer.init(p.timer0);
    port := p.uart0.open(&p.gpio, 115200);
    port.loopback(true);
    port.send(0x55);
    if port.receive() != 0x55 { rp2350.record.report(.trap(0)); }
    if timer.now() > 10 { rp2350.record.report(.trap(0)); }
    if input.low() { led.high(); }
    rp2350.nvic.disable(rp2350.irq.timer0_0);
    rp2350.nvic.clear(rp2350.irq.timer0_0);
    mut a := blink(&mut led, &timer);
    std.task.run(.{ &mut a })
};
";

#[test]
fn a_program_using_every_peripheral_builds_into_a_uf2_the_boot_rom_accepts() {
    let fixture = Fixture::new("rp2350-board");
    let dir = fixture.root.join("firmware");
    let mut session = fixture.session();
    session.new_project(&dir, "rp2350").unwrap();
    std::fs::write(dir.join("app.vun"), BOARD).unwrap();
    let mut session = fixture.session();
    let file = session.load(&dir.join(MANIFEST)).unwrap();
    let programs = session.carry_out(file).unwrap_or_else(|_| {
        let reports = session.diag.sorted(session.origins);
        panic!("{:#?}", reports.iter().map(|d| d.message.to_string()).collect::<Vec<_>>())
    });
    let [program] = programs.as_slice() else { panic!("one program is built: {programs:?}") };
    let [flashing] = program.flashing.as_slice() else { panic!("one file is flashed: {program:?}") };
    let bytes = std::fs::read(&flashing.file).unwrap();
    let (uf2, blocks) = vun_uf2::Uf2::read(&bytes).unwrap();
    assert_eq!(uf2.family, FAMILY);
    assert_eq!(blocks[0].address, FLASH);
}
