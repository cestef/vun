use fmt;

export fn main() void = {
	let state: u64 = 88172645463325252;
	let hash: u64 = 14695981039346656037;
	for (let i = 0u32; i < 40000000; i += 1) {
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		hash = (hash ^ state) * 1099511628211;
	};
	fmt::println(hash)!;
};
