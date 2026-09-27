use vun_diag::errors;

use super::{first, parse_text};

#[test]
fn lines_hold_a_mnemonic_and_its_operands() {
    assert_eq!(
        first("f := () { lang.arch.x86_64.asm {\n    mov rdi, rsp\n    syscall\n} };"),
        "(Binding':= f (Fn'( ) (Block'{ (Asm'. (Member'. (Member'. lang arch) x86_64) asm { \
         (Line'mov rdi , rsp) (Line'syscall) }) })))"
    );
}

#[test]
fn braces_bind_values_and_brackets_address_memory() {
    assert_eq!(
        first("f := () { a.asm {\n    lea {out r}, [{in(rsi) p} + 3]\n    add {x}, -1\n} };"),
        "(Binding':= f (Fn'( ) (Block'{ (Asm'. a asm { \
         (Line'lea (Bound'{ out r }) , (Memory'[ (Infix'+ (Bound'{ in ( rsi ) p }) 3) ])) \
         (Line'add (Bound'{ x }) , (Prefix'- 1)) }) })))"
    );
}

#[test]
fn flow_words_are_names_elsewhere() {
    assert_eq!(
        first("f := () { a.asm {\n    push {out}\n} };"),
        "(Binding':= f (Fn'( ) (Block'{ (Asm'. a asm { (Line'push (Bound'{ out })) }) })))"
    );
    assert_eq!(first("x := asm.out;"), "(Binding':= x (Member'. asm out))");
    assert_eq!(
        first("f := () { if c.asm { } };"),
        "(Binding':= f (Fn'( ) (Block'{ (If'if (Member'. c asm) (Block'{ })) })))"
    );
}

#[test]
fn operands_are_separated_by_commas() {
    let (_, ids) = parse_text("f := () { a.asm {\n    mov rdi rsp\n} };");
    assert_eq!(ids, [errors::SYNTAX_EXPECTED_TOKEN]);
}

#[test]
fn memory_writes_back_after_a_bang() {
    assert_eq!(
        first("f := () { a.asm {\n    ldr x0, [x1, 8]!\n    ldr x0, [x1], 8\n} };"),
        "(Binding':= f (Fn'( ) (Block'{ (Asm'. a asm { (Line'ldr x0 , (Memory'[ x1 , 8 ] !)) \
         (Line'ldr x0 , (Memory'[ x1 ]) , 8) }) })))"
    );
}

#[test]
fn labels_mark_lines_that_jumps_target() {
    assert_eq!(
        first("f := () { a.asm {\n'top:\n    dec rcx\n    b.ne 'top\n'end: ret\n} };"),
        "(Binding':= f (Fn'( ) (Block'{ (Asm'. a asm { 'top : (Line'dec rcx) (Line'b (Suffix'. ne) 'top) 'end : (Line'ret) }) })))"
    );
}

#[test]
fn lines_end_operands_and_keywords_spell_mnemonics() {
    assert_eq!(
        first("f := () { a.asm {\n    mov rdi, rsp\n    and rsp, -16\n} };"),
        "(Binding':= f (Fn'( ) (Block'{ (Asm'. a asm { (Line'mov rdi , rsp) (Line'and rsp , (Prefix'- 16)) }) })))"
    );
}
