# cli.unavailable_command

The command exists in the command tree but is not implemented yet.

The command line of spec 12, section 7 is declared whole, so help lists every command and marks
the ones still to come `(planned)`; running one reports this instead of doing nothing.

## Example

```console
$ overt pkg update
error[cli.unavailable_command]: `overt pkg update` is not available yet
```

## Fix

Use a command that is available; `overt pkg --help` marks the planned ones:

```console
$ overt pkg --help
dependencies, releases and API diffs

usage: overt pkg <command> [options]
       overt pkg help <command>

commands:
  add     declare a dependency, fetching an archive to learn its hash
  update  raise dependency versions (planned)
  fetch   download dependencies into the local store and write the lockfile
  ...
```
