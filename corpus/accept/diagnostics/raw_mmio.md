# Raw volatile stores

A volatile store outside `std.hw` writes a register without the abstraction that names it,
checks its width and orders it with the other accesses. Declare the register with `std.hw` and
write it through its methods.
