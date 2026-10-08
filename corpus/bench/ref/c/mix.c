#include <stdint.h>
#include <stdio.h>

int main(void) {
    uint64_t state = 88172645463325252ULL;
    uint64_t hash = 14695981039346656037ULL;
    for (uint32_t i = 0; i < 40000000; i++) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        hash = (hash ^ state) * 1099511628211ULL;
    }
    printf("%llu\n", (unsigned long long)hash);
    return 0;
}
