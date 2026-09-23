# coroutine.recursive_frame

A coroutine's frame would contain itself.

Calling a coroutine function constructs its frame, whose size is known at compile time, and
`yield ..e` constructs the frame of `e` in place inside the parent's frame (spec 06, sections 1
and 5). A coroutine that delegates to itself, directly or indirectly, would need an infinite frame,
so it is an error unless the recursion goes through an `Own` (spec 06, section 1).

## Example

```ov
Tree := struct { value: u32, children: &[Tree] };

walk := (t: &Tree) -> Iter(u32) { //~ coroutine.recursive_frame
    yield t.value;
    for c in t.children { yield ..walk(c); }
};
```

## Fix

Recurse in an ordinary function, whose frames live on the stack, or allocate each nested frame in
an `Own`:

```ov
Tree := struct { value: u32, children: &[Tree] };

sum := (t: &Tree) -> u64 {
    mut total: u64 = t.value;
    for c in t.children { total += sum(c); }
    total
};
```
