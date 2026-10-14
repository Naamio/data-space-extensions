# Benchmarks

The RDF component contract is designed on the claim that typed statements outperform passing
text for the host to parse again, and the claim has to be measured rather than assumed. This
crate measures it, and measures what writing an extension in TypeScript costs compared with
Rust.

## Method

Each timing turns one generated document into a dataset. Three paths read Turtle:

- **Typed.** A component runs the Turtle grammar and streams typed statements, as written,
  across the boundary. The extension host interprets them into the dataset. This is the
  contract.
- **Text.** The same component, running the same grammar, writes those statements back out
  as normalised Turtle: prefix and base directives, then one triple per line, with prefixed
  names, relative references and blank node labels exactly as written. The host parses the
  text with oxttl, which resolves it into the dataset.
- **Native.** The host parses the document itself with oxttl, with no component. This is the
  floor.

The typed and text paths run in one component binary, `turtle-text-parser`, which exports both
the contract's `parser` interface and the benchmark's `text-parser` interface. On both paths
the host does all the interpreting, so the only difference between them is the form the
statements take when they cross the boundary.

A second group reads N-Triples through the Rust kit's example, the JavaScript kit's example
and native oxttl.

Each document describes a number of people with the same ten triples about each, using the
constructs the host has to interpret: prefixed names, relative references, `a`, unquoted
numbers, square-bracket property lists, labelled blank nodes and language tags. The sizes are
one thousand, ten thousand and one hundred thousand triples.

Every timed run creates a new component instance and a new dataset, as the host does for each
document. Before timing, each path's result is checked against the number of triples the
document states, and `tests/paths.rs` checks that every path stores the same quads.

Run it with `cargo bench -p benchmarks`. The JavaScript example needs Node 22.15 or later and
pnpm, as the host's tests do. Criterion writes reports to `target/criterion`.

## Results

Recorded on 14 October 2026 on an Apple M1, with the `nightly-2026-10-10` toolchain,
Wasmtime 49.0.2, jco 1.37.0 and componentize-qjs 0.4.5. Each figure is criterion's median of
ten samples.

### Turtle

| Triples | Typed | Text | Native |
|---:|---:|---:|---:|
| 1,000 | 2.62 ms | 3.54 ms | 1.58 ms |
| 10,000 | 31.0 ms | 41.0 ms | 20.8 ms |
| 100,000 | 429 ms | 469 ms | 316 ms |

In throughput, the typed path reads 382,000, 323,000 and 233,000 triples a second at the three
sizes; the text path 282,000, 244,000 and 213,000; native oxttl 633,000, 482,000 and 317,000.

### N-Triples

| Triples | Rust kit | JavaScript kit | Native |
|---:|---:|---:|---:|
| 1,000 | 3.06 ms | 115 ms | 1.54 ms |
| 10,000 | 34.7 ms | 1.16 s | 19.6 ms |
| 100,000 | 466 ms | 12.3 s | 309 ms |

### Component sizes

| Component | Size |
|---|---:|
| `turtle-text-parser`, both exports | 323 KB |
| Rust kit's N-Triples example | 219 KB |
| JavaScript kit's N-Triples example | 1.9 MB |

## Findings

**Typed statements outperform text at every size.** The text path takes 35 per cent longer
than the typed path at one thousand triples, 32 per cent longer at ten thousand, and 10 per
cent longer at one hundred thousand. The criterion holds.

The gap narrows as documents grow. Both paths share the cost of inserting quads into the
dataset, and native oxttl's throughput halves between the smallest and largest documents too,
which points to insertion growing faster than linearly while the cost of the boundary format
stays proportional to the document. The benchmark does not profile the paths, so this
explanation is consistent with the figures rather than shown by them. The text path's timings
also vary more from sample to sample than the typed path's.

**The component paths cost 1.4 to 1.7 times native parsing.** That figure combines several
costs the benchmark does not separate: instantiating the component, crossing the boundary,
the component's grammar, which collects the whole document before parsing and is younger than
oxttl's, and the host's interpreter. Profiling them separately is the next step before
optimising any one.

**An extension written in TypeScript is 26 to 37 times slower than the same extension in
Rust.** The JavaScript kit's component runs in QuickJS, an interpreter without a just-in-time
compiler, and builds a JavaScript object for every term of every statement before lowering it
across the boundary. It is also nine times the size, because it carries the engine. TypeScript
suits small syntaxes and quick experiments; a syntax that handles large documents belongs in
Rust.

**Limits of these figures.** They come from one machine, one shape of document and ten samples
a point. Every component path parses the whole document before writing any statement, so none
of them overlaps parsing with ingestion.

## Wrong turn

The first design for the text path put oxttl inside a comparison component, which would
resolve the document itself and pass finished N-Quads to the host. It would not build for
`wasm32-wasip3`, because oxrdf's `rand` dependency pulls in getrandom 0.3, which supports
WebAssembly System Interface (WASI) previews 1 and 2 only. More importantly, it measured the
wrong alternative. A component that resolves its own document is the design an adversarial
review of the contract proposed and the contract rejected, not the contract with text in
place of typed values. The text path described above keeps the host owning meaning on both sides, needs no
RDF library in WebAssembly, and isolates the one difference the criterion is about.
