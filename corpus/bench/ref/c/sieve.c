#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#define N 4000000

int main(void) {
    bool composite[N] = {false};
    uint32_t count = 0;
    for (size_t i = 2; i < N; i++) {
        if (!composite[i]) {
            count++;
            for (size_t j = i * i; j < N; j += i) composite[j] = true;
        }
    }
    printf("%u\n", count);
    return 0;
}
