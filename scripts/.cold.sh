#!/usr/bin/env bash
# usage: .cold.sh NAME [build]   builds, primes a tree ~/cold/NAME, then times cold builds of sieve
set -e
name=$1
root=$HOME/cold/$name
if [ "${2:-}" = build ]; then
  cargo build --release -p vun-cli 2>&1 | tail -1
  rm -rf $root $root-store; mkdir -p $root/bin $HOME/cold/work
  cp target/release/vun $root/bin/vun; cp -r lib $root/lib
  VUN_STORE=$root-store $root/bin/vun toolchain prime >/dev/null 2>&1
fi
mkdir -p $HOME/cold/work; cp corpus/bench/sieve.vun $HOME/cold/work/
export VUN_STORE=$root-store
cd $HOME/cold/work
$root/bin/vun build sieve.vun -o out >/dev/null 2>&1
for i in $(seq 1 25); do
  rm -rf .vun
  s=$(date +%s%N); $root/bin/vun build sieve.vun -o out >/dev/null 2>&1; e=$(date +%s%N)
  echo $(( (e-s)/1000 ))
done | sort -n | awk '{a[NR]=$1} END {printf "%s min %.1f median %.1f ms\n", "'$name'", a[1]/1000, a[int(NR/2)+1]/1000}'
