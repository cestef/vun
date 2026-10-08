use fmt;

def N: size = 4000000;

export fn main() void = {
	let composite: [N]bool = [false...];
	let count: u32 = 0;
	for (let i = 2z; i < N; i += 1) {
		if (!composite[i]) {
			count += 1;
			for (let j = i * i; j < N; j += i) {
				composite[j] = true;
			};
		};
	};
	fmt::println(count)!;
};
