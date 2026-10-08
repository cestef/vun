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
	os.Stdout.WriteString(strconv.FormatUint(fib(34), 10) + "\n")
}
