mod asm;

use vun_diag::{DiagCx, DiagId, errors};

use super::MAX_NESTING;
use crate::cst::NodeId;
use crate::cst::{Anchor, Cst};
use crate::tokens::Tokens;
use vun_diag::DiagCtxt;
use vun_origin::OriginTable;
use vun_span::SourceFiles;

/// The tree as an s-expression: `(Kind'token children)` for branches, the token text for
/// leaves, `?Kind` for missing nodes.
fn sexp(cst: &Cst, tokens: &Tokens, text: &str, n: NodeId) -> std::string::String {
    let token = match cst.anchor(n) {
        Anchor::Token(t) => tokens.range(t).of(text).to_owned(),
        Anchor::Missing(_) => return format!("?{:?}", cst.kind(n)),
    };
    if cst.kind(n).is_leaf() {
        return token;
    }
    let children: Vec<_> = cst.children(n).map(|c| sexp(cst, tokens, text, c)).collect();
    let mut out = format!("({:?}'{token}", cst.kind(n));
    for child in children {
        out.push(' ');
        out.push_str(&child);
    }
    out.push(')');
    out
}

/// The top-level entries of `text` and the diagnostic ids, after checking the verifier.
/// The verified tree of `text`, its tokens, and the ids of what parsing reported.
fn parsed(text: &str) -> (Cst, Tokens, Vec<DiagId>) {
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    assert_eq!(cst.verify(&tokens), Ok(()), "{text:?}");
    (cst, tokens, diag.sorted(&origins).into_iter().map(|d| d.id.clone()).collect())
}

fn parse_text(text: &str) -> (Vec<std::string::String>, Vec<DiagId>) {
    let (cst, tokens, ids) = parsed(text);
    (cst.children(cst.root()).map(|n| sexp(&cst, &tokens, text, n)).collect(), ids)
}

fn clean(text: &str) -> Vec<std::string::String> {
    let (entries, ids) = parse_text(text);
    assert!(ids.is_empty(), "unexpected {ids:?} for {text:?}");
    entries
}

fn first(text: &str) -> std::string::String {
    clean(text).swap_remove(0)
}

#[test]
fn precedence_and_associativity() {
    assert_eq!(first("x := a + b * c - d;"), "(Binding':= x (Infix'- (Infix'+ a (Infix'* b c)) d))");
    assert_eq!(
        first("x := -a.b.*? or !c and d;"),
        "(Binding':= x (Infix'or (Prefix'- (Postfix'? (Postfix'.* (Member'. a b)))) (Infix'and (Prefix'! c) d)))"
    );
    assert_eq!(first("x := a..=b + 1;"), "(Binding':= x (Infix'..= a (Infix'+ b 1)))");
    assert_eq!(first("x := s[i..];"), "(Binding':= x (Index'[ s (Infix'.. i) ]))");
}

#[test]
fn groups_become_functions_by_what_follows() {
    assert_eq!(
        first("f := (a: u8) -> u8 { a };"),
        "(Binding':= f (Fn'( (Param': a u8) ) (Returns'-> u8) (Block'{ a })))"
    );
    assert_eq!(first("f := () { };"), "(Binding':= f (Fn'( ) (Block'{ })))");
    assert_eq!(
        first("f: (u8, bool) -> u8 = _;"),
        "(TypedBinding': f (FnType'( u8 , bool ) (Returns'-> u8)) = _)"
    );
    assert_eq!(first("t := (a, b);"), "(Binding':= t (Tuple'( a , b )))");
    assert_eq!(first("t := (a,);"), "(Binding':= t (Tuple'( a , )))");
    assert_eq!(first("t := ();"), "(Binding':= t (Tuple'( )))");
    assert_eq!(first("p := (a);"), "(Binding':= p (Paren'( a )))");
    assert_eq!(
        first("P := ($T: type) -> type { struct {} };"),
        "(Binding':= P (Fn'( (Param': (Prefix'$ T) type) ) (Returns'-> type) (Block'{ (Struct'struct { }) })))"
    );
    assert_eq!(
        first("f: unsafe (p: *u8) = _;"),
        "(TypedBinding': f (Prefix'unsafe (FnType'( (Param': p (Prefix'* u8)) ))) = _)"
    );
}

#[test]
fn brackets() {
    assert_eq!(first("a := [1, 2];"), "(Binding':= a (Array'[ 1 , 2 ]))");
    assert_eq!(first("a := [0; 4];"), "(Binding':= a (ArrayRepeat'[ 0 ; 4 ]))");
    assert_eq!(first("mut b: [N]u8;"), "(TypedBinding': mut b (ArrayType'[ N ] u8))");
    assert_eq!(
        first("b: &static [_]u8 = _;"),
        "(TypedBinding': b (Prefix'& static (ArrayType'[ _ ] u8)) = _)"
    );
    assert_eq!(first("m: [2][3]u8 = _;"), "(TypedBinding': m (ArrayType'[ 2 ] (ArrayType'[ 3 ] u8)) = _)");
    assert_eq!(
        first("c := [&mut n] (x: u8) { n += x; };"),
        "(Binding':= c (Closure'[ (Prefix'& mut n) ] (Fn'( (Param': x u8) ) (Block'{ (Assign'+= n x) ; }))))"
    );
    assert_eq!(first("c := [] () {};"), "(Binding':= c (Closure'[ ] (Fn'( ) (Block'{ }))))");
}

#[test]
fn literals() {
    assert_eq!(
        first("p := Point{ .x = 1 };"),
        "(Binding':= p (TypedLit'{ Point (FieldInit'= (DotName'. x) 1) }))"
    );
    assert_eq!(
        first("p := .{ .x = 1, 2 };"),
        "(Binding':= p (StructLit'.{ (FieldInit'= (DotName'. x) 1) , 2 }))"
    );
    assert_eq!(first("s := .some(3);"), "(Binding':= s (Call'( (DotName'. some) 3 )))");
    assert_eq!(first("E := enum { \\unsafe, \\x };"), "(Binding':= E (Enum'enum { \\unsafe , \\x }))");
    assert_eq!(first("r := \\\"C:\\dir\";"), "(Binding':= r \\\"C:\\dir\")");
    assert_eq!(first("E := enum { a, b(u8) };"), "(Binding':= E (Enum'enum { a , (Call'( b u8 )) }))");
    assert_eq!(
        first("q := #{ $x * $x };"),
        "(Binding':= q (Quote'#{ (Infix'* (Prefix'$ x) (Prefix'$ x)) }))"
    );
    assert_eq!(first("s := \\\\ a\n    \\\\ b\n;"), "(Binding':= s (MultilineString'\\\\ a \\\\ b))");
}

#[test]
fn heads_end_at_the_body() {
    assert_eq!(
        first("f := () { if a == B { c } else if d { e } };"),
        "(Binding':= f (Fn'( ) (Block'{ (If'if (Infix'== a B) (Block'{ c }) else (If'if d (Block'{ e }))) })))"
    );
    assert_eq!(
        first("f := () { for x in xs { } for .some(j) = q.pop() { } for n < 3 { } for { } };"),
        "(Binding':= f (Fn'( ) (Block'{ (For'for (In'in x xs) (Block'{ })) (For'for (Test'= (Call'( (DotName'. some) j )) (Call'( (Member'. q pop) ))) (Block'{ })) (For'for (Infix'< n 3) (Block'{ })) (For'for (Block'{ })) })))"
    );
    assert_eq!(
        first("f := () { match c { .a(x) if x > 0 => x, _ => {}, } };"),
        "(Binding':= f (Fn'( ) (Block'{ (Match'match c { (Arm'=> (Call'( (DotName'. a) x )) (Guard'if (Infix'> x 0)) x) , (Arm'=> _ (Block'{ })) , }) })))"
    );
}

#[test]
fn statements() {
    assert_eq!(
        first("f := () { if a { b } (c, d) := e; 'l: for { break 'l x; } return; };"),
        "(Binding':= f (Fn'( ) (Block'{ (If'if a (Block'{ b })) (Binding':= (Tuple'( c , d )) e) ; (Labeled''l : (For'for (Block'{ (Break'break 'l x) ; }))) (Return'return) ; })))"
    );
    assert_eq!(
        first("f := () { _ = g(&out b); defer h(); };"),
        "(Binding':= f (Fn'( ) (Block'{ (Assign'= _ (Call'( g (Prefix'& out b) ))) ; (Defer'defer (Call'( h ))) ; })))"
    );
    // `defer` runs a whole statement: an assignment binds looser than the operand it defers.
    assert_eq!(
        first("f := () { defer x.* += 1; };"),
        "(Binding':= f (Fn'( ) (Block'{ (Defer'defer (Assign'+= (Postfix'.* x) 1)) ; })))"
    );
    assert_eq!(
        first("f := () { n := yield ..s.read(); unsafe { g(); } };"),
        "(Binding':= f (Fn'( ) (Block'{ (Binding':= n (Yield'yield (Prefix'.. (Call'( (Member'. s read) ))))) ; (Prefix'unsafe (Block'{ (Call'( g )) ; })) })))"
    );
}

#[test]
fn members_fields_and_annotations() {
    assert_eq!(
        first("S := struct { x: u8, y: u8 = 0, d := 1; };"),
        "(Binding':= S (Struct'struct { (Field': x u8) , (Field': y u8 = 0) , (Binding':= d 1) ; }))"
    );
    assert_eq!(
        first("S := struct { $for r in rs { #[a(1), b] $(r.name): u8, } };"),
        "(Binding':= S (Struct'struct { (Prefix'$ (For'for (In'in r rs) (Block'{ (Attr'#[ (Call'( a 1 )) , b ] (Field': (Prefix'$ (Paren'( (Member'. r name) ))) u8)) , }))) }))"
    );
    let entries = clean("//! m\n/// d\n#[t] x := 1;");
    assert_eq!(entries, ["//! m", "(Doc'/// d (Attr'#[ t ] (Binding':= x 1)))", ";"]);
}

#[test]
fn errors_are_reported_once_and_the_tree_verifies() {
    let cases: &[(&str, &[DiagId])] = &[
        ("x := 1\ny := 2;", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := ;", &[errors::SYNTAX_EXPECTED_EXPRESSION]),
        ("x := f(a, b;", &[errors::SYNTAX_UNCLOSED_DELIMITER]),
        ("x := f(a b);", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := a < b < c;", &[errors::SYNTAX_CHAINED_OPERATOR]),
        ("x := a.;", &[errors::SYNTAX_EXPECTED_NAME]),
        ("f(x);", &[errors::SYNTAX_EXPECTED_TOKEN]),
        (") x := 1;", &[errors::SYNTAX_EXPECTED_DECLARATION]),
        ("x := { a b };", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := @;", &[errors::SYNTAX_UNKNOWN_CHARACTER]),
        ("x := match a { .b c, };", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := struct { a b };", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := (", &[errors::SYNTAX_UNCLOSED_DELIMITER]),
        ("/// dangling", &[errors::SYNTAX_EXPECTED_DECLARATION]),
    ];
    for &(text, expected) in cases {
        assert_eq!(parse_text(text).1, expected, "{text:?}");
    }
}

#[test]
fn deep_nesting_is_bounded() {
    let depth = MAX_NESTING as usize * 2;
    let text = format!("x := {}1{};", "(".repeat(depth), ")".repeat(depth));
    assert_eq!(parse_text(&text).1, [errors::SYNTAX_NESTING_TOO_DEEP]);
    let text = format!("x := {}1;", "-".repeat(depth));
    assert_eq!(parse_text(&text).1, [errors::SYNTAX_NESTING_TOO_DEEP]);
}

#[test]
fn long_chains_are_bounded() {
    let links = MAX_NESTING as usize * 64;
    for text in [
        format!("x := 1{};", " + 1".repeat(links)),
        format!("x := a{};", ".b".repeat(links)),
        format!("x := f{};", "()".repeat(links)),
        format!("x := if a {{}}{} else {{}};", " else if a {}".repeat(links)),
    ] {
        let (cst, _, ids) = parsed(&text);
        assert_eq!(ids, [errors::SYNTAX_NESTING_TOO_DEEP], "{}", &text[..40]);
        assert!(cst.depth() <= MAX_NESTING as usize + 8, "depth {}", cst.depth());
    }
}

#[test]
fn recovered_trees_fit_their_slots() {
    // Inputs the fuzzer found: junk in a list, a dangling doc before a failed entry, and brackets
    // before a group that is neither a closure nor an array type.
    for text in ["[)#[", "///\n'''", "[](", "[a, b](c)"] {
        let (_, ids) = parse_text(text);
        assert!(!ids.is_empty(), "{text:?} should report");
    }
}

#[test]
fn fields_name_the_children() {
    use vun_grammar::{NodeField as F, NodeKind as K};
    let text = "mut f: T = (a: u8, b) -> u8 { a };";
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    let binding = cst.children(cst.root()).next().unwrap();
    let fields = cst.fields(binding).unwrap();
    let kind = |f| fields.one(f).map(|n| cst.kind(n));
    assert_eq!([kind(F::Mut), kind(F::Pattern), kind(F::Type)], [Some(K::Mut), Some(K::Name), Some(K::Name)]);
    let function = cst.fields(fields.one(F::Value).unwrap()).unwrap();
    let params: Vec<_> = function.get(F::Params).iter().map(|&n| cst.kind(n)).collect();
    assert_eq!(params, [K::Param, K::Name]);
    assert_eq!(function.one(F::Returns).map(|n| cst.kind(n)), Some(K::Returns));
    assert_eq!(function.one(F::Block).map(|n| cst.kind(n)), Some(K::Block));
    assert!(function.get(F::Statements).is_empty());
}

#[test]
fn borrow_modes_and_pattern_forms() {
    assert_eq!(
        first("f := (a: &temp mut T) { g(&mut out); };"),
        "(Binding':= f (Fn'( (Param': a (Prefix'& temp mut T)) ) (Block'{ (Call'( g (Prefix'& mut out) )) ; })))"
    );
    assert_eq!(
        first("f := () { match e { n := .add(l, ..) => n, .a | .b => 0, 'a'..='z' => 1, _ => 2 } };"),
        "(Binding':= f (Fn'( ) (Block'{ (Match'match e { (Arm'=> (Binding':= n (Call'( (DotName'. add) l , .. ))) n) , (Arm'=> (Infix'| (DotName'. a) (DotName'. b)) 0) , (Arm'=> (Infix'..= 'a' 'z') 1) , (Arm'=> _ 2) }) })))"
    );
    assert_eq!(
        first("f := () { if n := .some(x) = e { } };"),
        "(Binding':= f (Fn'( ) (Block'{ (If'if (Test'= (Binding':= n (Call'( (DotName'. some) x ))) e) (Block'{ })) })))"
    );
}

#[test]
fn spreads() {
    assert_eq!(first("..lang.int;"), "(Spread'.. (Member'. lang int))");
    assert_eq!(first("..std.{ mem, \\type };"), "(Spread'.. std (Select'.{ mem , \\type }))");
    assert_eq!(
        first("S := struct { ..a; x: u8, };"),
        "(Binding':= S (Struct'struct { (Spread'.. a) ; (Field': x u8) , }))"
    );
    assert_eq!(parse_text("..a.{ 1 };").1, [errors::SYNTAX_EXPECTED_NAME]);
}
