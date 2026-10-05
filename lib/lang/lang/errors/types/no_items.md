# types.no_items

A `$for` is given a value it cannot step through.

A `$for` unrolls at compile time (spec 03, section 1): its body is typed and built once per item,
with the binder bound to the item. The items are those of an array, of a tuple, each typed by its
own, or of a range. Anything else, a number, a record, a value of a type with its own iteration,
has no items to unroll over.

## Example

```vun
sum := () -> u32 {
    mut total: u32 = 0;
    $for x in 5 { //~ types.no_items
        total += x;
    }
    total
};
```

## Fix

Step through what has items, here the range up to the number:

```vun
sum := () -> u32 {
    mut total: u32 = 0;
    $for x in 0..5 {
        total += x;
    }
    total
};
```

A loop over a value known only when the program runs is a `for`, without `$`.
