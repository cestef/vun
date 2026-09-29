#[test]
fn members_of_types_have_handles_by_their_path() {
    assert_eq!(crate::std::build_Build::new.path(), ["std", "build", "Build", "new"]);
    assert_eq!(crate::std::build_Action::dep.to_string(), "std.build.Action.dep");
    assert_eq!(crate::lang::target_Endian::little.name(), "little");
}
