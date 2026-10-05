// Illustrations: diagrams drawn with cetz, plots with lilaq. Data shown here is stated in the text
// that refers to each figure.

#import "@preview/cetz:0.4.2"
#import "@preview/lilaq:0.5.0" as lq
#import "template.typ": accent, small-size

#let ink = luma(35)
#let faint = luma(150)
#let panel = luma(246)
#let highlight = accent.lighten(80%)
#let muted-ink = luma(90)

#let label-text(body) = text(small-size, fill: ink, body)
#let mono(body) = text(font: ("JetBrains Mono", "DejaVu Sans Mono"), size: 7.6pt, ligatures: false, body)

// A labeled box; `planned` draws it dashed and grey.
#let node(draw, at, name, body, width: 2.2, height: 0.9, planned: false, fill: panel) = {
  let (x, y) = at
  draw.rect(
    (x - width / 2, y - height / 2), (x + width / 2, y + height / 2),
    name: name,
    radius: 0.08,
    fill: if planned { white } else { fill },
    stroke: if planned { (paint: faint, dash: "dashed", thickness: 0.6pt) } else { 0.6pt + ink },
  )
  draw.content(name, text(small-size, fill: if planned { faint } else { ink }, body))
}

#let arrow(draw, from, to, planned: false, label: none, anchor: "south", padding: 0.12) = {
  draw.line(
    from, to,
    mark: (end: "stealth", fill: if planned { faint } else { ink }, scale: 0.7),
    stroke: if planned { (paint: faint, dash: "dashed", thickness: 0.6pt) } else { 0.6pt + ink },
  )
  if label != none {
    draw.content((from, 50%, to), anchor: anchor, padding: padding, text(7.8pt, fill: muted-ink, label))
  }
}

// The layering rule: syntax desugars into lang; std and user code are built on it.
#let layering = cetz.canvas(length: 1cm, {
  import cetz.draw
  node(draw, (0, 0), "syntax", [syntax], width: 2.1)
  node(draw, (3.9, 0), "lang", [#mono[lang]], width: 2.1, fill: highlight)
  node(draw, (7.8, 0), "std", [#mono[std]], width: 2.1)
  node(draw, (11.7, 0), "user", [user code], width: 2.1)
  arrow(draw, "syntax.east", "lang.west", label: [desugars into])
  arrow(draw, "std.west", "lang.east", label: [built on])
  arrow(draw, "user.west", "std.east", label: [built on])
  draw.content((3.9, -0.8), text(7.8pt, fill: muted-ink)[primitives: `= _` declarations])
})

// Place states and the transitions borrows and moves perform (table of borrow forms).
#let place-states = cetz.canvas(length: 1cm, {
  import cetz.draw
  let state(at, name, body, sub) = {
    draw.circle(at, radius: 0.62, name: name, fill: panel, stroke: 0.7pt + ink)
    draw.content(name, text(12pt, style: "italic", body))
    draw.content((at.at(0), at.at(1) - 0.95), text(7.8pt, fill: muted-ink, sub))
  }
  state((0, 0), "u", $U$, [uninitialized])
  state((6, 0), "i", $I$, [initialized])
  let bend(from, via, to, label, anchor) = {
    draw.bezier(from, to, via, mark: (end: "stealth", fill: ink, scale: 0.7), stroke: 0.6pt + ink)
    draw.content(via, anchor: anchor, padding: 0.05, text(7.8pt, label))
  }
  bend("u.north-east", (3, 1.35), "i.north-west", [initialization, `&out`, assignment in place], "south")
  bend("i.south-west", (3, -1.35), "u.south-east", [move of a linear value, `&take`, `end`], "north")
  // Self-loops.
  draw.bezier("i.north-east", "i.south-east", (7.9, 1.3), (7.9, -1.3),
    mark: (end: "stealth", fill: ink, scale: 0.7), stroke: 0.6pt + ink)
  draw.content((8.05, 0), anchor: "west", text(7.8pt)[`&`, `&mut`, `&temp`, copy, \ read, assignment])
  draw.bezier("u.north-west", "u.south-west", (-1.9, 1.3), (-1.9, -1.3),
    mark: (end: "stealth", fill: ink, scale: 0.7), stroke: 0.6pt + ink)
  draw.content((-2.05, 0), anchor: "east", text(7.8pt)[declaration \ without a value])
  draw.content((3, -2.35), text(7.8pt, fill: muted-ink)[at every scope exit, every linear place is in $U$])
})

// Frame of `echo` (section on suspension): the delegated read and write frames share a slot.
#let frame-layout = cetz.canvas(length: 1cm, {
  import cetz.draw
  let cell(x0, x1, y0, y1, body, fill: panel) = {
    draw.rect((x0, y0), (x1, y1), fill: fill, stroke: 0.6pt + ink)
    draw.content(((x0 + x1) / 2, (y0 + y1) / 2), text(7.8pt, body))
  }
  cell(0, 1.8, 0, 1.4, [parameter \ `s`])
  cell(1.8, 2.9, 0, 1.4, [state])
  cell(2.9, 6.4, 0, 1.4, [`buf: [64]u8` \ live at both points], fill: highlight)
  cell(6.4, 13.2, 0.7, 1.4, [frame of `s.read(&out buf)`: suspension 1])
  cell(6.4, 13.2, 0, 0.7, [frame of `s.write(&buf[0..n])`: suspension 2])
  let brace(x0, x1, y, body, anchor) = {
    draw.line((x0, y), (x1, y), stroke: 0.4pt + faint, mark: (start: "|", end: "|", scale: 0.5))
    draw.content(((x0 + x1) / 2, y), anchor: anchor, padding: 0.1, text(7.8pt, fill: muted-ink, body))
  }
  brace(6.4, 13.2, 1.6, [one slot, reused by both delegations], "south")
  brace(0, 13.2, -0.2, [`std.meta.Frame(echo)`, whose size is known at compile time], "north")
})

// The compiler pipeline.
#let pipeline = cetz.canvas(length: 1cm, {
  import cetz.draw
  let w = 2.0
  let step = 3.4
  let row = (0, 1, 2, 3).map(i => i * step)
  node(draw, (row.at(0), 0), "src", [source], width: w)
  node(draw, (row.at(1), 0), "cst", [tokens, CST], width: w)
  node(draw, (row.at(2), 0), "hir", [HIR], width: w)
  node(draw, (row.at(3), 0), "mir", [MIR], width: w, fill: highlight)
  node(draw, (row.at(3), 2.1), "interp", [interpreter], width: w)
  node(draw, (row.at(3), -2.2), "lir", [LIR], width: w)
  node(draw, (row.at(2), -2.2), "back", [backend], width: w)
  node(draw, (row.at(1), -2.2), "link", [linker], width: w)
  node(draw, (row.at(0), -2.2), "out", [executable], width: w)
  arrow(draw, "src.east", "cst.west", label: [parse])
  arrow(draw, "cst.east", "hir.west", label: [desugar, \ resolve])
  arrow(draw, "hir.east", "mir.west", label: [type, \ evaluate])
  arrow(draw, "mir.north", "interp.south", label: [runs], anchor: "west")
  draw.line("interp.west", (row.at(2), 2.1), "hir.north",
    mark: (end: "stealth", fill: ink, scale: 0.7), stroke: 0.6pt + ink)
  draw.content((row.at(2) + 0.1, 2.1), anchor: "south-west", padding: 0.1,
    text(7.8pt, fill: muted-ink)[values and types])
  arrow(draw, "mir.south", "lir.north", label: [lower], anchor: "west")
  arrow(draw, "lir.west", "back.east")
  arrow(draw, "back.west", "link.east", label: [objects])
  arrow(draw, "link.west", "out.east", label: [link plan])
  draw.content((row.at(2), -2.75), anchor: "north", text(7.8pt, fill: muted-ink)[C, native, wasm])
  draw.content((row.at(1), -2.75), anchor: "north", text(7.8pt, fill: muted-ink)[GNU ld, mold, wild, lld, wasm])
  draw.content((row.at(3) + w / 2 + 0.2, 0), anchor: "west",
    text(7.8pt, fill: muted-ink)[checks: \ places, kinds, \ borrows, traps])
})

// Reserved words (section on syntax).
#let keyword-counts = {
  let names = ("Vun", "Lua", "Go", "Rust", "C", "Zig")
  let counts = (15, 22, 25, 39, 44, 49)
  let xs = range(names.len())
  lq.diagram(
    width: 9cm, height: 3.6cm,
    ylabel: [reserved words],
    ylim: (0, 55),
    xlim: (-0.6, 5.6),
    xaxis: (ticks: xs.zip(names), subticks: none),
    yaxis: (subticks: none),
    lq.bar(xs.slice(1), counts.slice(1), fill: luma(200), stroke: 0.4pt + luma(120), width: 0.55),
    lq.bar((0,), (counts.at(0),), fill: accent.lighten(35%), stroke: 0.4pt + accent, width: 0.55),
  )
}

// The product of two ranges attains its extremes at the corners of the box of operands.
#let interval-product = {
  let a = (-3, 4)
  let b = (-2, 5)
  let steps = 57
  let xs = lq.linspace(a.at(0), a.at(1), num: steps)
  let ys = lq.linspace(b.at(0), b.at(1), num: steps)
  let corners = a.map(x => b.map(y => (x, y))).flatten().chunks(2)
  let tag(x, y) = lq.place(
    x, y,
    pad(4pt, box(fill: white, inset: 2pt, radius: 1pt, text(7.5pt, fill: ink, str(x * y)))),
    align: (if x < 0 { left } else { right }) + (if y < 0 { bottom } else { top }),
  )
  lq.diagram(
    width: 6.4cm, height: 5.4cm,
    xlabel: $a$, ylabel: $b$,
    xaxis: (subticks: none), yaxis: (subticks: none),
    lq.colormesh(xs, ys, (x, y) => x * y, map: color.map.vlag),
    lq.scatter(corners.map(c => c.at(0)), corners.map(c => c.at(1)), size: 6pt, color: ink),
    ..corners.map(((x, y)) => tag(x, y)),
  )
}

// Lines of Rust per workspace group (section on implementation).
#let code-size = {
  // Listed bottom to top, so the layering order reads from the top.
  let groups = (
    "tools", "pkg", "native", "render", "link", "back", "asm", "low", "mid", "sema", "front",
    "object", "isa", "lang", "base",
  )
  let lines = (
    37214, 3752, 7327, 716, 2155, 11481, 2086, 15887, 6853, 20445, 10298, 5797, 5735, 1543, 21251,
  )
  let ys = range(groups.len())
  lq.diagram(
    width: 9cm, height: 7.4cm,
    xlabel: [lines of Rust],
    yaxis: (ticks: ys.zip(groups.map(g => raw(g))), subticks: none),
    xaxis: (ticks: (0, 10000, 20000, 30000, 40000).map(v => (v, str(v))), subticks: none),
    xlim: (0, 40000),
    ylim: (-0.6, 14.6),
    lq.hbar(lines, ys, fill: accent.lighten(40%), stroke: 0.4pt + accent, width: 0.6),
  )
}
