package main

import (
	"os"
	"strconv"
)

func main() {
	var state uint64 = 88172645463325252
	var hash uint64 = 14695981039346656037
	for i := uint32(0); i < 40000000; i++ {
		state ^= state << 13
		state ^= state >> 7
		state ^= state << 17
		hash = (hash ^ state) * 1099511628211
	}
	os.Stdout.WriteString(strconv.FormatUint(hash, 10) + "\n")
}
