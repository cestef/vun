# types.mismatch

A value's type is not the type expected where it is used.

Types are values, and every `struct` and `enum` literal is a new nominal type: two identical
literals in different declarations are different types and never mix (spec 02, section 2). Aliases
stay structural, and the only implicit acceptance between distinct types is widening an integer
range (spec 04, section 1, rule 2) and `&static T` for `&T` (spec 05, section 4).

## Example

```ov
NodeId := struct { raw: u32 };
TypeId := struct { raw: u32 };

type_of := (n: NodeId) -> TypeId {
    n //~ types.mismatch
};
```

## Fix

Build a value of the expected type explicitly:

```ov
NodeId := struct { raw: u32 };
TypeId := struct { raw: u32 };

type_of := (n: NodeId) -> TypeId {
    TypeId{ .raw = n.raw }
};
```
