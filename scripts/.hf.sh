#!/usr/bin/env bash
cd $HOME/cold/work
for r in 1 2; do for n in "$@"; do
  VUN_STORE=$HOME/cold/$n-store hyperfine -w 3 -r 40 -N --prepare "rm -rf $HOME/cold/work/.vun" "$HOME/cold/$n/bin/vun build sieve.vun -o out" 2>&1 | grep -E "Time|Range" | sed "s/^/$n /"
done; done
