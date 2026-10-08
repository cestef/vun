use crate::Entropy;
use crate::draw::{Config, Draw};
use crate::eval::Machine;
use crate::lower::Spelling;

#[test]
fn every_shrink_of_a_program_makes_another_one_that_the_model_runs() {
    let config = Config::default();
    let program = Draw::program(&mut Entropy::seeded(3), &config);
    let source = program.source(Spelling::ALL[0]);
    let count = program.shrinks();
    assert!(count > 10);
    for nth in (0..count).step_by(7) {
        let smaller = program.shrunk(nth).expect("a counted site applies");
        assert_ne!(smaller.source(Spelling::ALL[0]), source, "shrink {nth} changed nothing");
        let _ = Machine::new(&smaller, config.fuel()).hashes();
    }
    assert!(program.shrunk(count).is_none());
}
