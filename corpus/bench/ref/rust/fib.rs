fn fib(n: u32) -> u64 {
    if n < 2 { u64::from(n) } else { fib(n - 1).wrapping_add(fib(n - 2)) }
}

fn main() {
    let n = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(34);
    println!("{}", fib(n));
}
