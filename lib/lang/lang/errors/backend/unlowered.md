# backend.unlowered

The program cannot be built: a function it reaches does not lower to LIR yet.

Building runs every function the root file's exports reach through lowering (spec 13, section
3): each must be concrete MIR, and each of its forms one lowering handles. The diagnostic says
which form stopped it, at the place it is written. `vun debug lir` shows the same reason, and
`vun debug program` which functions the program reaches.

## Example

```console
$ vun build iter.vun
error[backend.unlowered]: the program does not lower to LIR: lowering to LIR does not handle coroutine frames yet
```

## Fix

Write the program without the form until lowering handles it, or follow `vun debug program` to
the function that reaches it:

```console
$ vun debug program iter.vun
```
