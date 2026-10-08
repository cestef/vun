#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static uint64_t fib(uint32_t n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }

int main(int argc, char **argv) {
    uint32_t n = argc > 1 ? (uint32_t)strtoul(argv[1], 0, 10) : 34;
    printf("%llu\n", (unsigned long long)fib(n));
    return 0;
}
