# asm.stack

An instruction of a stack machine takes or leaves the stack wrong.

Each instruction of a stack machine's table declares the values it pops and the values it pushes
(spec 07, section 6). A block of them starts with an empty stack and ends with one, so each value
an instruction pops was pushed before it, as the type it takes, and none is left at the end. After an
instruction that ends the code (`unreachable`) any stack goes.

## Example

```vun
add := (a: i32, b: i32) {
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(a));
        i32_add(); //~ asm.stack
    });
};
```

## Fix

Push the second operand, and pop the result into a place:

```vun
add := (a: i32, b: i32) {
    mut r: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(a));
        local_get(in(b));
        i32_add();
        local_set(out(r));
    });
};
```
