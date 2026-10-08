const N: usize = 4_000_000;

fn main() {
    let mut composite = [false; N];
    let mut count: u32 = 0;
    for i in 2..N {
        if !composite[i] {
            count = count.wrapping_add(1);
            let mut j = i.wrapping_mul(i);
            while j < N {
                composite[j] = true;
                j = j.wrapping_add(i);
            }
        }
    }
    println!("{count}");
}
