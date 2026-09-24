// Vun whitepaper template: one column, academic, generous spacing.

#let accent = rgb("#1f3a5f")
#let rule-color = luma(120)
#let muted = luma(95)
#let terminal-bg = rgb("#1c1f24")
#let terminal-fg = rgb("#d5d8dc")

#let body-size = 10.8pt
#let small-size = 9.2pt
#let code-size = 8.3pt
#let term-size = 7.6pt

// The heading of the current page's section, for the running header.
#let current-section() = {
  let before = query(heading.where(level: 1).before(here()))
  let after = query(heading.where(level: 1).after(here()))
  let page = here().page()
  let starting = after.filter(h => h.location().page() == page)
  let head = if starting.len() > 0 { starting.first() } else if before.len() > 0 { before.last() } else { none }
  if head == none { return none }
  if head.numbering == none { head.body } else [#numbering(head.numbering, ..counter(heading).at(head.location()))#h(0.6em)#head.body]
}

#let paper(title: "", subtitle: "", authors: (), date: none, abstract: [], keywords: (), body) = {
  set document(title: title, author: authors.map(a => a.name), keywords: keywords)
  set page(
    paper: "a4",
    margin: (inside: 3.1cm, outside: 3.1cm, top: 3.3cm, bottom: 3.2cm),
    header-ascent: 38%,
    footer-descent: 40%,
    header: context {
      if here().page() > 1 {
        set text(8pt, fill: muted, tracking: 0.02em)
        grid(
          columns: (1fr, 1fr),
          align: (left, right),
          smallcaps(lower(title)), smallcaps(lower(current-section())),
        )
        v(-0.55em)
        line(length: 100%, stroke: 0.4pt + luma(185))
      }
    },
    footer: context {
      set text(8.5pt, fill: muted)
      align(center, counter(page).display("1"))
    },
  )

  set text(font: "Libertinus Serif", size: body-size, lang: "en", hyphenate: true, number-type: "old-style")
  set par(justify: true, leading: 0.78em, spacing: 1.25em, first-line-indent: (amount: 1.4em, all: false))
  show math.equation: set text(font: "New Computer Modern Math", number-type: "lining")
  show strong: set text(weight: "semibold")

  // Headings: numbered, black, small caps at the top level.
  set heading(numbering: "1.1")
  show heading: set text(weight: "semibold", number-type: "lining")
  show heading.where(level: 1): it => {
    v(2.4em, weak: true)
    block(sticky: true, below: 1.2em, text(12.5pt, tracking: 0.03em, {
      if it.numbering != none { counter(heading).display(it.numbering); h(0.9em) }
      smallcaps(it.body)
    }))
  }
  show heading.where(level: 2): it => {
    v(1.8em, weak: true)
    block(sticky: true, below: 0.9em, text(11pt, {
      if it.numbering != none { counter(heading).display(it.numbering); h(0.8em) }
      it.body
    }))
  }
  show heading.where(level: 3): it => {
    v(1.3em, weak: true)
    block(sticky: true, below: 0.7em, text(body-size, style: "italic", weight: "regular", it.body))
  }

  // Code: a quiet panel with room around it.
  // Code: a quiet panel with room around it. Terminal sessions (`term`) are shown as the
  // terminal shows them, on a dark panel with the renderer's colors.
  set raw(syntaxes: ("vun.sublime-syntax", "term.sublime-syntax"))
  set raw(theme: "vun.tmTheme")
  show raw.where(lang: "term"): set raw(theme: "term.tmTheme")
  show raw: set text(
    font: ("JetBrains Mono", "DejaVu Sans Mono"),
    number-type: "lining",
    ligatures: false,
    features: (calt: 0),
  )
  show raw.where(block: true): set text(size: code-size)
  show raw.where(block: false): set text(size: 1em)
  show raw.where(lang: "term"): set text(size: term-size, fill: terminal-fg)
  show raw.where(block: false): box.with(inset: (x: 0.6pt), outset: (y: 1.5pt))
  show raw.where(block: true): it => {
    set par(justify: false, leading: 0.62em)
    let term = it.lang == "term"
    v(0.9em, weak: true)
    block(
      width: 100%,
      fill: if term { terminal-bg } else { luma(248) },
      inset: (x: 12pt, y: 10pt),
      radius: if term { 3pt } else { 0pt },
      stroke: if term { none } else { (top: 0.5pt + luma(205), bottom: 0.5pt + luma(205)) },
      breakable: true,
      align(left, it),
    )
    v(0.9em, weak: true)
  }

  // Figures and tables in booktabs style.
  set figure(gap: 1em)
  show figure: set block(above: 1.7em, below: 1.7em)
  show figure.caption: it => {
    set text(small-size)
    set par(justify: true, first-line-indent: 0em)
    block(width: 100%, inset: (x: 0.8em))[#smallcaps[*#it.supplement #context it.counter.display(it.numbering).*] #it.body]
  }
  show figure.where(kind: table): set figure.caption(position: top)
  set table(stroke: none, inset: (x: 7pt, y: 5.5pt), align: left + top)
  show table: set text(small-size, number-type: "lining")
  show table: set par(justify: false)
  show table.cell.where(y: 0): set text(weight: "semibold")

  set list(indent: 1em, body-indent: 0.6em, spacing: 0.9em)
  set enum(indent: 1em, body-indent: 0.6em, spacing: 0.9em)
  set terms(indent: 1em, hanging-indent: 1.4em, spacing: 0.9em)
  show link: set text(fill: accent)
  show ref: set text(fill: accent)
  show cite: set text(fill: accent)
  set footnote.entry(separator: line(length: 25%, stroke: 0.4pt + luma(160)))
  show footnote.entry: set text(8.8pt)

  // Title block.
  v(1.6cm)
  align(center)[
    #text(26pt, weight: "semibold", tracking: 0.04em, smallcaps(title))
    #v(0.5em)
    #text(13pt, style: "italic", subtitle)
    #v(1.8em)
    #for a in authors [
      #text(11pt, smallcaps(a.name))#if "affiliation" in a [ \ #text(9.5pt, style: "italic", a.affiliation)] \
    ]
    #if date != none [#v(0.4em) #text(9.5pt, fill: muted, date)]
  ]
  v(2.4em)
  pad(x: 1.6cm)[
    #set text(9.8pt)
    #set par(first-line-indent: 0em, leading: 0.72em)
    #align(center, smallcaps[*Abstract*])
    #v(0.5em)
    #abstract
    #if keywords.len() > 0 [
      #v(0.9em)
      #text(9pt)[#smallcaps[*Keywords.*] #keywords.join(", ")]
    ]
  ]
  pagebreak()

  // Contents, on a page of its own.
  {
    v(1.2cm)
    align(center, text(11pt, tracking: 0.03em, smallcaps[*Contents*]))
    v(1.2em)
    show outline.entry.where(level: 1): set text(weight: "semibold")
    show outline.entry.where(level: 1): set block(above: 1.1em)
    set text(9.8pt, number-type: "lining")
    pad(x: 1.2cm, outline(title: none, depth: 2, indent: 1.8em))
  }
  pagebreak()

  body
}

// Numbered, theorem-style statement of a language rule.
#let rule-counter = counter("rule")
#let rule(name, body) = {
  rule-counter.step()
  block(width: 100%, inset: (x: 1.2em), above: 1.3em, below: 1.3em, breakable: false)[
    #set par(first-line-indent: 0em)
    #smallcaps[*Rule #context rule-counter.display()*] #h(0.2em) (#name). #h(0.2em) #emph(body)
  ]
}

// An aside set off from the running text.
#let note(body) = {
  v(0.5em, weak: true)
  block(
    width: 100%, inset: (left: 12pt, right: 6pt, y: 6pt),
    stroke: (left: 0.8pt + luma(160)),
    { set par(first-line-indent: 0em); text(small-size, body) },
  )
  v(0.5em, weak: true)
}

// Top, middle and bottom rules for booktabs tables.
#let toprule = table.hline(stroke: 0.9pt)
#let midrule = table.hline(stroke: 0.45pt)
#let bottomrule = table.hline(stroke: 0.9pt)

// Appendices: lettered headings after this point.
#let appendix(body) = {
  counter(heading).update(0)
  set heading(numbering: "A.1", supplement: [Appendix])
  body
}
