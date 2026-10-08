# test.unmatched

`vun test`, `vun bench`, `vun fuzz` or `vun cover` was asked for tests, by name or by a text their
names hold, and the namespace declares none that matches.

The names after the path (`vun test main.vun crc_vector`) and `--filter` choose among the tests
the namespace declares (spec 11). Choosing none is an error, not a run of zero tests, so a typo
cannot pass a check. A namespace that declares no tests at all is not an error: the command says
so and succeeds. The help suggests the closest declared name, or lists them.

## Example

```console
$ vun test main.vun crc_vecter
error[test.unmatched]: no test is named `crc_vecter`
  │
  ╰ help: did you mean `crc_vector`?
```

## Fix

Name a test the namespace declares, or a text its name holds:

```console
$ vun test main.vun crc_vector
$ vun test --filter crc
```
