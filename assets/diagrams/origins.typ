// The origin chain: an entity of each stage points back to what it was made from, down to the span
// of source text. The lines are what `vun debug tokens|hir|mir|lir|code` print for `a + b` in
// `corpus/accept/programs/tour.vun` on x86-64, the MIR line shortened to its operands.

#import "diagram.typ": node, pipeline, span

#let code = "mid := (a: u32, b: u32) -> u32 { (a + b) / 2 };"
#let picked = "a + b"

#let diagram(t) = {
  let from = code.position(picked)
  pipeline(t, dir: ttb, back: true, length: 1.6, note: "origin", (
    span(t, code, from, from + picked.len(), note: "tour.vun 7:34, bytes 156..161"),
    (body: "tokens", args: (note: "Ident 156..157 \"a\"  Plus 158..159 \"+\"")),
    (body: "hir", args: (note: "Infix + @7:34")),
    (body: "mir", args: (note: "_5 = lang.int.add(_3, _4) @7:34")),
    (body: "lir", args: (note: "v4 = add v2, v3 @7:34")),
    (body: "code", args: (note: "add rsi, rcx  ; tour.vun:7", kind: "accent")),
  ))
}
