# binding.missing_case

A `match` does not cover every value of what it matches.

The arms of a `match` cover every value of the scrutinee's type: enums by variant, integers by
range, tuples and structs by their parts (spec 01, section 6). An arm with a guard covers nothing,
since its guard may fail. The error names one value no arm matches.

## Example

```vun
Light := enum { red, amber, green };

wait := (l: Light) -> u8 {
    match l { //~ binding.missing_case
        .red => 30,
        .green => 0,
    }
};
```

## Fix

Add an arm for what is missing, or `_ =>` for every value left:

```vun
Light := enum { red, amber, green };

wait := (l: Light) -> u8 {
    match l {
        .red => 30,
        .amber => 5,
        .green => 0,
    }
};
```
