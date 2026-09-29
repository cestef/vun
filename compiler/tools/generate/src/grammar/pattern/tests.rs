use super::Pattern;

#[test]
fn a_literal_escapes_what_a_pattern_reads_otherwise() {
    assert_eq!(Pattern::literal("..=").to_string(), r"\.\.=");
    assert_eq!(Pattern::literal("#{").to_string(), r"#\{");
}

#[test]
fn words_try_the_longest_first() {
    assert_eq!(Pattern::words(["in", "inout", "out"]).to_string(), r"\b(?:inout|out|in)\b");
}
