use overt_hash::Hash;

use super::{Lock, Locked, Malformed};
use crate::manifest::{Identity, Version};

fn entry(name: &str, version: &str, identity: Option<Identity>, via: Option<&str>) -> Locked {
    Locked {
        name: name.into(),
        version: Version::parse(version).unwrap(),
        identity,
        hash: Hash::of(name.as_bytes()),
        via: via.map(str::to_owned),
    }
}

#[test]
fn locks_round_trip_in_canonical_order() {
    let hal = Identity { id: 0xc0ff_ee01, major: 2 };
    let lock = Lock::new(vec![
        entry("stm32f4", "1.3.0", Some(Identity { id: 0x6a80_91f5_7c7f_07ff, major: 1 }), None),
        entry("hal", "2.1.4", Some(hal), Some("stm32f4")),
        entry("cmsis", "5.9.0", None, None),
    ]);
    let text = lock.to_string();
    let names: Vec<_> = lock.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["cmsis", "hal", "stm32f4"]);
    assert!(text.contains(" 00000000c0ffee01/2 "), "{text}");
    assert!(text.lines().nth(2).unwrap().ends_with(" via stm32f4"), "{text}");
    assert_eq!(Lock::parse(&text), Ok(lock.clone()));
    assert_eq!(lock.selected(hal).map(|e| e.version), Version::parse("2.1.4"));
    let older = Lock::new([lock.entries(), &[entry("hal", "2.0.0", Some(hal), None)]].concat());
    assert_eq!(older.selected(hal).map(|e| e.version), Version::parse("2.1.4"), "the largest is selected");
}

#[test]
fn malformed_locks_name_the_line() {
    assert_eq!(Lock::parse("dep a 1.0.0 foreign x"), Err(Malformed::Header));
    let lock = Lock::new(vec![entry("a", "1.0.0", None, None)]).to_string();
    assert_eq!(Lock::parse(&format!("{lock}dep b 1.0 foreign x\n")), Err(Malformed::Line(3)));
    assert_eq!(Identity::parse("c0ffee01/2"), None, "ids are written in full");
    assert_eq!(Identity::parse("00000000c0ffee01/02"), None);
}
