// Overt whitepaper template: minimal, academic, one column.

#let accent = rgb("#1f3a5f")

#let paper(title: "", subtitle: "", authors: (), date: none, abstract: [], keywords: (), body) = {
  set document(title: title, author: authors.map(a => a.name))
  set page(
    paper: "a4",
    margin: (x: 2.6cm, top: 2.8cm, bottom: 2.8cm),
    numbering: "1",
    number-align: center,
    header: context {
      if counter(page).get().first() > 1 [
        #set text(8pt, fill: luma(90))
        #smallcaps(title) #h(1fr) #smallcaps(subtitle)
      ]
    },
  )
  set text(font: "Libertinus Serif", size: 10.5pt, lang: "en")
  set par(justify: true, leading: 0.62em, spacing: 1.05em, first-line-indent: (amount: 1.2em, all: false))
  show math.equation: set text(font: "New Computer Modern Math")

  set heading(numbering: "1.1")
  show heading: set text(fill: accent, weight: "semibold")
  show heading.where(level: 1): it => {
    v(1.4em, weak: true)
    block(text(13pt, it))
    v(0.6em, weak: true)
  }
  show heading.where(level: 2): it => { v(1em, weak: true); block(text(11pt, it)); v(0.4em, weak: true) }
  show heading.where(level: 3): it => { v(0.8em, weak: true); block(text(10.5pt, style: "italic", it.body)) }

  set raw(syntaxes: "overt.sublime-syntax")
  show raw: set text(font: "DejaVu Sans Mono", size: 8.4pt)
  show raw.where(block: true): it => block(
    width: 100%,
    fill: luma(247),
    inset: (x: 9pt, y: 7pt),
    radius: 2pt,
    stroke: (left: 1.5pt + accent.lighten(55%)),
    it,
  )

  set figure(gap: 0.8em)
  show figure.caption: set text(9pt)
  show figure.where(kind: table): set figure.caption(position: top)
  set table(stroke: none, inset: (x: 6pt, y: 4pt))
  show table: set text(9pt)
  show table.cell.where(y: 0): set text(weight: "semibold")

  set list(indent: 0.8em, spacing: 0.7em)
  set enum(indent: 0.8em, spacing: 0.7em)
  show link: set text(fill: accent)
  show ref: set text(fill: accent)

  // Title block
  align(center)[
    #v(1.2cm)
    #text(22pt, weight: "semibold", fill: accent, title)
    #v(0.3em)
    #text(12pt, style: "italic", subtitle)
    #v(1em)
    #for a in authors [#a.name#if "affiliation" in a [, #text(style: "italic", a.affiliation)] \ ]
    #if date != none [#v(0.3em) #text(9pt, date)]
  ]
  v(1.2em)
  pad(x: 1.2cm)[
    #set text(9.5pt)
    #set par(first-line-indent: 0em)
    #smallcaps[*Abstract.*] #abstract
    #if keywords.len() > 0 [
      #v(0.4em)
      #smallcaps[*Keywords.*] #keywords.join(", ")
    ]
  ]
  v(1.5em)

  body
}

// A numbered definition-like block.
#let rule-counter = counter("rule")
#let rule(name, body) = {
  rule-counter.step()
  block(width: 100%, inset: (y: 3pt))[
    #smallcaps[*Rule #context rule-counter.display()*] (#name). #body
  ]
}

#let note(body) = block(
  width: 100%, inset: (x: 9pt, y: 6pt),
  stroke: (left: 0.8pt + luma(160)),
  text(9.5pt, body),
)
