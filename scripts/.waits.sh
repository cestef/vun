#!/usr/bin/env bash
n=$1
export VUN_STORE=$HOME/cold/$n-store
cd $HOME/cold/work; rm -rf .vun
VUN_LOG=vun_query=debug $HOME/cold/$n/bin/vun build sieve.vun -o out 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep "waited for another" | sed -E 's/.*query="?([a-z_]+)"?.*waited=([0-9.]+)(.s|ms).*/\1 \2 \3/' | awk '{v=$2; if ($3=="µs") v=v/1000; n[$1]++; t[$1]+=v} END {for (k in n) printf "%s %d %.1f\n", k, n[k], t[k]}' | sort -k3 -n -r | head
