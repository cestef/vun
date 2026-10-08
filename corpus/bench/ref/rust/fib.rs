fn fib(n: u32) -> u64 {
    if n < 2 { u64::from(n) } else { fib(n - 1).wrapping_add(fib(n - 2)) }
}

fn main() {
    println!("{}", fib(34));
}
