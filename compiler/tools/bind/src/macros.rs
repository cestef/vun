//! Object-like macros whose replacement is an integer constant, such as `#define LIMIT (0x10U)`
//! or `#define NEG -1`, become constants; any other macro is not bound.

/// The value of the replacement `tokens`: an integer literal in C's forms (decimal, `0x`, `0b`,
/// octal, with `u` and `l` suffixes), optionally negated and parenthesized.
pub(crate) fn integer(tokens: &[String]) -> Option<i128> {
    match tokens {
        [open, inner @ .., close] if open == "(" && close == ")" => integer(inner),
        [minus, rest @ ..] if minus == "-" => integer(rest)?.checked_neg(),
        [literal] => literal_value(literal),
        _ => None,
    }
}

fn literal_value(literal: &str) -> Option<i128> {
    let digits = literal.trim_end_matches(['u', 'U', 'l', 'L']);
    let (digits, radix) = if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        (hex, 16)
    } else if let Some(bin) = digits.strip_prefix("0b").or_else(|| digits.strip_prefix("0B")) {
        (bin, 2)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (&digits[1..], 8)
    } else {
        (digits, 10)
    };
    // C allows `'` between digits since C23.
    i128::from_str_radix(&digits.replace('\'', ""), radix).ok()
}

#[cfg(test)]
#[test]
fn integer_macros_read_in_every_form_c_writes() {
    let tokens = |ts: &[&str]| ts.iter().map(|&t| t.to_owned()).collect::<Vec<_>>();
    assert_eq!(integer(&tokens(&["(", "0x10U", ")"])), Some(16));
    assert_eq!(integer(&tokens(&["-", "1"])), Some(-1));
    assert_eq!(integer(&tokens(&["010"])), Some(8));
    assert_eq!(integer(&tokens(&["0b101"])), Some(5));
    assert_eq!(integer(&tokens(&["1'000ULL"])), Some(1000));
    assert_eq!(integer(&tokens(&["1.5"])), None);
    assert_eq!(integer(&tokens(&["A", "+", "1"])), None);
    assert_eq!(integer(&[]), None);
}
