use fmt;

fn fib(n: u32) u64 = {
	if (n < 2) {
		return n;
	};
	return fib(n - 1) + fib(n - 2);
};

export fn main() void = {
	fmt::println(fib(34))!;
};
