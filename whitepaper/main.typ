#import "template.typ": *

#show: paper.with(
  title: "Overt",
  subtitle: "A systems language with nothing behind the scenes",
  authors: ((name: "The Overt authors"),),
  date: "Draft, September 2026",
  abstract: [
    Overt is a systems programming language for code that runs under constraints: firmware,
    kernels, runtimes, and servers that care about memory and latency. Its design follows a single
    rule: the language consists of seven concepts, each governed by one rule, and every other
    feature is a library built on them or a documented rewrite into them. Primitives are ordinary
    declarations in a source package; syntax desugars only into that package; the standard library
    holds no privilege. We present the concepts (bindings, compile-time values, range integers,
    place states, scoped borrows, capabilities and suspension), show how they remove the usual
    special cases of systems languages (integer overflow, lifetime annotations, destructors,
    pinning, entry-point magic, macro languages), and describe an implementation architecture in
    which every stage after the front end, from code generator to linker, is replaceable, and every
    entity in every intermediate representation remains traceable to the source text that caused it.
  ],
  keywords: ("systems programming", "language design", "staging", "linear types", "range types", "embedded systems"),
)

#include "sections/01-introduction.typ"
#include "sections/02-principles.typ"
#include "sections/03-syntax.typ"
#include "sections/04-staging.typ"
#include "sections/05-integers.typ"
#include "sections/06-memory.typ"
#include "sections/07-suspension.typ"
#include "sections/08-capabilities.typ"
#include "sections/09-performance.typ"
#include "sections/10-tooling.typ"
#include "sections/11-implementation.typ"
#include "sections/12-related.typ"
#include "sections/13-conclusion.typ"

#bibliography("refs.yml", style: "association-for-computing-machinery")
