use std::path::PathBuf;

use vun_intern::Symbol;
use vun_link::{LinkPlan, MemoryRegion, OutputKind, SectionPlacement};
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};

use crate::Script;
use crate::script::{Inputs, OutputSection, Pattern, Region};

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
    assert_eq!(Script::of(&plan), expected);
}
