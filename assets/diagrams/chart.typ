// Charts of the measurements in `assets/bench/results.json` (written by `scripts/bench-compare`), in
// the diagrams' style and from the same themes: Vun in the accent, the others in the muted color.
// A filled bar is a build with optimizations, an outlined bar one without; a whisker spans the
// quartiles of the samples.
//
// Imported from the repository's root, `/assets/diagrams/chart.typ`, by the benchmark card and
// the whitepaper.

#import "diagram.typ": label, pen

#let data = json("/assets/bench/results.json")

// What each program is, as the charts name it.
#let titles = (fib: "fib(34)", sieve: "sieve, 4M", mix: "xorshift, 40M")
#let remarks = (mix: "a serial chain: builds tie")

#let seconds-text(s) = {
  let ms = s * 1000
  if ms >= 100 { str(calc.round(ms, digits: 0)) + " ms" } else if ms >= 10 {
    str(calc.round(ms, digits: 1)) + " ms"
  } else { str(calc.round(ms, digits: 2)) + " ms" }
}

#let variants = data.results.map(r => r.variant).dedup()
#let entry(variant, program) = data.results.find(r => r.variant == variant and r.program == program)
#let language-of(variant) = data.results.find(r => r.variant == variant).language

// One bar: `value` and the quartile span `(q1, q3)` in seconds, `filled` or outlined, `accent` or
// muted, `thin` for a secondary bar of the same row.
#let bar(value, span: none, filled: true, accent: false, thin: false) = (
  value: value, span: span, filled: filled, accent: accent, thin: thin,
)

// The mean of `field` of a variant's entries over the programs.
#let mean-of(variant, field) = {
  let all = data.programs.map(p => field(entry(variant, p)))
  all.sum() / all.len()
}

// A measure's `(median, q1, q3)` of a variant for a program, or the means over the programs when
// `program` is none.
#let stat(variant, program, pick) = if program == none {
  ("median", "q1", "q3").map(k => mean-of(variant, e => pick(e).at(k)))
} else {
  ("median", "q1", "q3").map(k => pick(entry(variant, program)).at(k))
}

// The bars a variant gets for a program in `measure`: "run" (one) or "compile" (cold, then warm
// for the toolchains that keep a cache).
#let bars(variant, program, measure) = {
  let e = entry(variant, data.programs.first())
  let filled = e.level != "none"
  let accent = e.language == "vun"
  let make(pick, thin: false) = {
    let (m, lo, hi) = stat(variant, program, pick)
    bar(m, span: (lo, hi), filled: filled, accent: accent, thin: thin)
  }
  if measure == "run" { (make(e => e.run),) } else {
    (make(e => e.compile.cold),) + if "warm" in e.compile { (make(e => e.compile.warm, thin: true),) } else { () }
  }
}

// A row's bars scaled so `top` fills `width`, the values written after them.
#let row-bars(t, series, top, width) = {
  let u = t.unit
  let scale(v) = v / top * width
  let drawn = series.map(b => {
    let h = if b.thin { u * 0.45 } else { u * 0.8 }
    let role = if b.accent { "accent" } else { "muted" }
    let w = calc.max(scale(b.value), u * 0.15)
    box(width: width + u * 5, height: h, {
      place(left + horizon, rect(
        width: w, height: h, radius: u * 0.15,
        fill: if b.filled { rgb(t.at(role)) } else { none },
        stroke: pen(t, role: role),
      ))
      if b.span != none {
        place(left + horizon, dx: scale(b.span.at(0)), line(length: scale(b.span.at(1)) - scale(b.span.at(0)), stroke: (paint: rgb(t.text), thickness: u * 0.07, cap: "butt")))
      }
      let end = if b.span == none { w } else { calc.max(w, scale(b.span.at(1))) }
      place(left + horizon, dx: end + u * 1.1, label(t, seconds-text(b.value), role: if b.accent { "accent" } else { "muted" }, size: 0.7))
    })
  })
  stack(spacing: u * 0.15, ..drawn)
}

// One panel per program, rows for the variants, a gap where the language changes. `measure` is
// "run" or "compile"; `shown` limits the variants.
#let chart(t, measure, programs, shown: variants, panel: 11) = {
  let u = t.unit
  let width = panel * u
  let top = programs.map(p => calc.max(..shown.map(name => bars(name, p, measure).map(b => b.value)).flatten())).map(m => m * 1.0)

  let cells = ()
  cells.push([])
  for p in programs {
    let title = if p == none { "mean of the three programs" } else { titles.at(p) }
    let remark = if p != none and p in remarks { (label(t, remarks.at(p), role: "muted", size: 0.65),) } else { () }
    cells.push(stack(spacing: u * 0.25, label(t, title, role: "muted", size: 0.85), ..remark))
  }
  let previous = none
  for name in shown {
    let language = language-of(name)
    if previous != none and language != previous {
      cells.push(grid.cell(colspan: programs.len() + 1, v(u * 0.2)))
    }
    previous = language
    cells.push(align(right + horizon, label(t, name, role: if language == "vun" { "accent" } else { "text" }, size: 0.8, weight: if language == "vun" { "bold" } else { "regular" })))
    for (i, p) in programs.enumerate() { cells.push(align(horizon, row-bars(t, bars(name, p, measure), top.at(i), width))) }
  }
  grid(columns: (auto, ..programs.map(_ => auto)), column-gutter: u * 1.2, row-gutter: u * 0.55, ..cells)
}

// A swatch of what a bar looks like, and what it says.
#let swatch(t, kind) = {
  let u = t.unit
  let (w, h) = if kind == "thin" { (u * 1.4, u * 0.45) } else { (u * 1.4, u * 0.8) }
  let role = if kind == "accent" { "accent" } else { "muted" }
  if kind == "whisker" {
    box(width: u * 1.4, height: u * 0.8, place(horizon, line(length: u * 1.4, stroke: (paint: rgb(t.text), thickness: u * 0.07))))
  } else {
    box(width: w, height: u * 0.8, align(horizon, rect(
      width: w, height: h, radius: u * 0.15,
      fill: if kind == "outlined" { none } else { rgb(t.at(role)) },
      stroke: pen(t, role: role),
    )))
  }
}

// A row of swatches with their meaning, then `setup`, the one line of how it was measured.
#let key(t, items, setup) = stack(
  spacing: t.unit * 0.9,
  stack(dir: ltr, spacing: t.unit * 1.8, ..items.map(((kind, text)) => stack(
    dir: ltr, spacing: t.unit * 0.5, swatch(t, kind), align(horizon, label(t, text, role: "muted", size: 0.7)),
  ))),
  label(t, setup, role: "muted", size: 0.7),
)

// `footnotes` false leaves the key to a caption.
#let run(t, panel: 11, footnotes: true) = stack(
  spacing: t.unit * 1.6,
  chart(t, "run", data.programs, panel: panel),
  ..if footnotes {
    (key(
      t,
      (("filled", "optimized"), ("outlined", "unoptimized"), ("accent", "vun"), ("whisker", "quartiles")),
      [median of #data.machine.samples runs, lower is better],
    ),)
  },
)

#let compile(t, panel: 18, footnotes: true) = stack(
  spacing: t.unit * 1.6,
  chart(t, "compile", (none,), panel: panel),
  ..if footnotes {
    (key(
      t,
      (("filled", "empty caches"), ("thin", "unchanged rebuild")),
      [median of #data.results.first().compile.cold.n builds, lower is better],
    ),)
  },
)
