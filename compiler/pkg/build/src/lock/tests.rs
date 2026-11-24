use vun_hash::Hash;

use super::{Lock, Locked, Pinned, Replaced, Replacing, Unreadable};
use crate::manifest::{Identity, Version};

fn entry(name: &str, version: &str, identity: Option<Identity>, via: Option<&str>) -> Locked {
    Locked {
        name: name.into(),
        version: version.parse::<Version>().unwrap(),
        identity,
        hash: Hash::of(name.as_bytes()),
        via: via.map(str::to_owned),
    }
}

#[test]
fn locks_round_trip_one_entry_per_line() {
    let hal = Identity { id: 0xc0ff_ee01, major: 2 };
    let lock = Lock::new(vec![
        entry("stm32f4", "1.3.0", Some(Identity { id: 0x6a80_91f5_7c7f_07ff, major: 1 }), None),
        entry("hal", "2.1.4", Some(hal), Some("stm32f4")),
        entry("cmsis", "5.9.0", None, None),
    ]);
    let text = lock.to_string();
    let names: Vec<_> = lock.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["cmsis", "hal", "stm32f4"]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[1], "version := 1;", "{text}");
    assert!(lines[3].contains(".id = .foreign"), "{text}");
    assert!(lines[4].contains(".id = .{ .id = 3237998081, .major = 2 }"), "{text}");
    assert!(lines[4].ends_with(r#".via = "stm32f4" },"#), "{text}");
    assert_eq!(text.parse::<Lock>(), Ok(lock.clone()));
    assert_eq!(lock.selected(hal).map(|e| e.version), "2.1.4".parse::<Version>().ok());
    let older = Lock::new([lock.entries(), &[entry("hal", "2.0.0", Some(hal), None)]].concat());
    assert_eq!(
        older.selected(hal).map(|e| e.version),
        "2.1.4".parse::<Version>().ok(),
        "the largest is selected"
    );
}

#[test]
fn malformed_locks_do_not_read() {
    let lock = Lock::new(vec![entry("a", "1.0.0", None, None)]).to_string();
    assert_eq!(
        lock.replace("version := 1", "version := 2").parse::<Lock>(),
        Err(Unreadable),
        "another layout"
    );
    assert_eq!(lock.replace("1.0.0", "1.0").parse::<Lock>(), Err(Unreadable));
    assert_eq!(lock.replace(".name", ".nmae").parse::<Lock>(), Err(Unreadable));
    assert_eq!("deps := .{};".parse::<Lock>(), Err(Unreadable), "the layout is required");
    assert_eq!("version := 1;\ndeps := .{};".parse::<Lock>(), Ok(Lock::default()));
}

#[test]
fn replacements_round_trip() {
    let hal = Identity { id: 0xc0ff_ee01, major: 2 };
    let lock = Lock::new(vec![entry("hal", "2.1.4", Some(hal), None)]).replacing(vec![
        Replaced { name: "hal".into(), identity: hal, with: Replacing::Path("../hal fork".into()) },
        Replaced {
            name: "sdk".into(),
            identity: Identity { id: 9, major: 0 },
            with: Replacing::Stored(Hash::of(b"sdk")),
        },
    ]);
    let text = lock.to_string();
    assert!(
        text.contains(
            r#".{ .name = "hal", .id = .{ .id = 3237998081, .major = 2 }, .path = "../hal fork" },"#
        ),
        "{text}"
    );
    assert_eq!(text.parse::<Lock>(), Ok(lock.clone()));
    assert_eq!(lock.replacement(hal).map(|r| &r.with), Some(&Replacing::Path("../hal fork".into())));
}

#[test]
fn a_pinned_toolchain_round_trips_before_the_entries() {
    let pinned = Pinned { version: "0.4.2".parse::<Version>().unwrap(), hash: Hash::of(b"release.json") };
    let lock = Lock::new(vec![entry("a", "1.0.0", None, None)]).pinning(Some(pinned));
    let text = lock.to_string();
    let at = |decl: &str| text.lines().position(|l| l.starts_with(decl));
    assert!(at("toolchain :=").is_some_and(|t| at("deps :=").is_some_and(|d| t < d)), "{text}");
    assert_eq!(text.parse::<Lock>(), Ok(lock.clone()));
    assert_eq!(lock.toolchain(), Some(pinned));
    assert!(!Lock::default().pinning(Some(pinned)).is_empty());
}
