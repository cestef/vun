package main

import (
	"os"
	"strconv"
)

func fib(n uint32) uint64 {
	if n < 2 {
		return uint64(n)
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	n := uint64(34)
	if len(os.Args) > 1 {
		if v, err := strconv.ParseUint(os.Args[1], 10, 32); err == nil {
			n = v
		}
	}
	os.Stdout.WriteString(strconv.FormatUint(fib(uint32(n)), 10) + "\n")
}
