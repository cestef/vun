use std::path::PathBuf;

use vun_intern::Symbol;
use vun_link::{LinkPlan, MemoryRegion, OutputKind, SectionPlacement};
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::script::{Inputs, OutputSection, Pattern, Region};
use crate::{Script, Unscripted};

#[test]
fn regions_and_sections_become_a_script() {
    let origins = OriginTable::new();
    let declared = origins.source(Span::new(SourceFiles::new().add("build.vun", "x"), 0, 1));
    let name = Symbol::intern;
    let placed = |section, region, load: Option<&str>| SectionPlacement {
        name: name(section),
        region: name(region),
        load_region: load.map(name),
        keep: false,
    };
    let plan = LinkPlan {
        inputs: vec![PathBuf::from("a.o")],
        output: PathBuf::from("a"),
        kind: OutputKind::Executable,
        entry: Some(name("Reset")),
        regions: vec![MemoryRegion { name: name("flash"), origin: 0x0800_0000, len: 0x10_0000, declared }],
        sections: vec![placed(".data", "flash", Some("flash"))],
    };
    let data = OutputSection {
        name: name(".data"),
        inputs: Inputs {
            keep: false,
            patterns: vec![Pattern::Exact(name(".data")), Pattern::Under(name(".data"))],
        },
        region: name("flash"),
        load: Some(name("flash")),
    };
    let expected = Script {
        entry: Some(name("Reset")),
        memory: vec![Region { name: name("flash"), origin: 0x0800_0000, length: 0x10_0000 }],
        sections: vec![data],
    };
    assert_eq!(Script::of(&plan), Ok(expected));
}

#[test]
fn names_a_script_cannot_say_are_refused() {
    let origins = OriginTable::new();
    let declared = origins.source(Span::new(SourceFiles::new().add("build.vun", "x"), 0, 1));
    let name = Symbol::intern;
    let region = |n| MemoryRegion { name: name(n), origin: 0, len: 0x100, declared };
    let plan = |regions, sections| LinkPlan {
        inputs: Vec::new(),
        output: PathBuf::from("a"),
        kind: OutputKind::Executable,
        entry: None,
        regions,
        sections,
    };
    let injected = name("ram) INCLUDE(/etc/passwd");
    assert_eq!(
        Script::of(&plan(vec![region("ram) INCLUDE(/etc/passwd")], Vec::new())),
        Err(Unscripted::Name(injected))
    );
    let placed =
        SectionPlacement { name: name(".data"), region: name("ram"), load_region: None, keep: false };
    let unplaced = Unscripted::Region { section: name(".data"), region: name("ram") };
    assert_eq!(Script::of(&plan(vec![region("flash")], vec![placed])), Err(unplaced));
}
