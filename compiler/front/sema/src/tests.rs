use crate::Primitive;
use crate::primitives::{lang, mem};

#[test]
fn primitives_are_the_top_level_holes_of_lang_by_namespace() {
    assert!(Primitive::ALL.contains(&Primitive::Mem(mem::Mem::Ref)));
    assert_eq!((Primitive::Lang(lang::Lang::Int).namespace(), lang::Lang::Int.name()), ("lang", "Int"));
    assert_eq!(Primitive::Lang(lang::Lang::r#type).name(), "type");
    // Requirements of interfaces are not primitives: `lang.ops.Add.add` is a hole inside a call.
    assert!(Primitive::ALL.iter().all(|p| p.namespace() != "lang.ops"), "{:?}", Primitive::ALL);
}
