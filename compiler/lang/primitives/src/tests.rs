use crate::{Primitive, lang, mem};

#[test]
fn primitives_are_the_top_level_holes_of_lang_by_namespace() {
    assert!(Primitive::all().any(|p| p == Primitive::Mem(mem::Mem::Ref)));
    assert_eq!(Primitive::Lang(lang::Lang::Int).item(), vun_items::lang::Int);
    assert_eq!(Primitive::Lang(lang::Lang::Int).namespace(), vun_items::lang);
    assert_eq!(Primitive::Lang(lang::Lang::r#type).name(), "type");
    // Requirements of interfaces are not primitives: `lang.ops.Add.add` is a hole inside a call.
    assert!(Primitive::all().all(|p| p.namespace() != vun_items::lang::ops));
}
