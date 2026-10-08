package main

import (
	"os"
	"strconv"
)

const n = 4000000

func main() {
	var composite [n]bool
	var count uint32
	for i := 2; i < n; i++ {
		if !composite[i] {
			count++
			for j := i * i; j < n; j += i {
				composite[j] = true
			}
		}
	}
	os.Stdout.WriteString(strconv.FormatUint(uint64(count), 10) + "\n")
}
