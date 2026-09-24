#import "template.typ": *

#show: paper.with(
  title: "Vun",
  subtitle: "Design and implementation of a systems programming language",
  authors: ((name: "cstef"),),
  date: "Draft of September 2026",
  abstract: [
    Vun is a systems programming language for software that runs under resource constraints:
    firmware, kernels, language runtimes and servers where memory use and latency matter. The
    design is organized around seven concepts, each governed by one rule: bindings, compile-time
    values, range integers, place states, scoped borrows, capabilities and suspension. Every other
    feature is either a library built on these concepts or a documented rewrite into them.
    Primitives are ordinary declarations in a source package, `lang`; syntax desugars only into
    that package, through tables that are themselves Vun data; and the standard library holds no
    privilege over user code. We describe the concepts and show how they account for features
    that systems languages usually treat as separate mechanisms: overflow behavior, lifetime
    annotations, destructors, pinning, entry points and macro systems. We then describe an
    implementation architecture in which every stage after the front end is replaceable, the
    compile-time interpreter defines the reference semantics, and every entity in every
    intermediate representation carries an origin chain to the source text that produced it. The
    front end, the compile-time evaluator and the mid-level checks are implemented; code
    generation is not yet. We report the state of the implementation and the questions that
    remain open.
  ],
  keywords: (
    "systems programming", "language design", "multi-stage programming", "linear types",
    "range types", "ownership and borrowing", "embedded systems",
  ),
)

#include "sections/01-introduction.typ"
#include "sections/02-principles.typ"
#include "sections/03-syntax.typ"
#include "sections/04-types.typ"
#include "sections/05-staging.typ"
#include "sections/06-integers.typ"
#include "sections/07-memory.typ"
#include "sections/08-suspension.typ"
#include "sections/09-capabilities.typ"
#include "sections/10-performance.typ"
#include "sections/11-tooling.typ"
#include "sections/12-implementation.typ"
#include "sections/13-related.typ"
#include "sections/14-conclusion.typ"

#bibliography("refs.yml", title: "References", style: "association-for-computing-machinery")

#show: appendix
#include "sections/15-appendix.typ"
