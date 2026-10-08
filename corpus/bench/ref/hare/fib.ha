use fmt;
use os;
use strconv;

fn fib(n: u32) u64 = {
	if (n < 2) {
		return n;
	};
	return fib(n - 1) + fib(n - 2);
};

export fn main() void = {
	let n: u32 = 34;
	if (len(os::args) > 1) {
		n = strconv::stou32(os::args[1]) as u32;
	};
	fmt::println(fib(n))!;
};
