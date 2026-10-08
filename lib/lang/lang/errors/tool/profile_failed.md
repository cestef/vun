# tool.profile_failed

`vun debug profile` could not measure a workload, or could not read or write a snapshot.

A measurement runs the compiler on a program of the corpus, timed, or under `valgrind` to count
instructions. It fails when the program does not start (`valgrind` is not installed), when the
compiler ends without success or runs past `--limit`, or when what it wrote is not a profile. The
cause follows the diagnostic as a note.

## Example

```console
$ vun debug profile check-tour --measure instructions
error[tool.profile_failed]: cannot measure `check-tour`
  = note: `valgrind` does not start
```

## Fix

Install the missing program, or measure the time instead:

```console
$ vun debug profile check-tour --measure time
```
