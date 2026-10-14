# Naamio Data Space Extensions

The extension host for [Naamio Data Space](https://naamio.space), and the Resource
Description Framework (RDF) data model it owns on behalf of the components it runs. This
repository is a Rust workspace. The host runs WebAssembly components on WebAssembly System
Interface (WASI) 0.3, and the `linked-data` crate interprets and stores the RDF they
produce.

## Design

A component knows one syntax, and the host owns meaning. A parser component streams back
the statements a document makes, as written: Internationalized Resource Identifiers
(IRIs) that may be relative or prefixed, and blank nodes referred to by labels. The
`linked-data` crate resolves the IRIs, expands the prefixes, gives each blank node its
identity, checks every term, and stores the result in a dataset. Everything common to
more than one syntax is written once, here, and no component carries its own copy.

Naamio Data Space is zero-knowledge: its sync and storage engine handles only ciphertext.
Extensions are the opposite, because parsing, validating and indexing happen only where
data has been decrypted. The engine therefore runs this host but does not depend on its
crates, and the separation is kept by crate dependencies, not by policy.

The contract between the host and its components is `linked-data:rdf`, from the
[Naamio Data Space WASI specifications](https://github.com/Naamio/data-space-wasi),
included as a Git submodule. The extension kits for
[Rust](https://github.com/Naamio/data-space-kit-rust) and
[JavaScript](https://github.com/Naamio/data-space-kit-javascript), and the
[Turtle parser](https://github.com/Naamio/data-space-turtle-parser), are submodules too.
The host's feature tests build them and run them end to end, so a change to the contract,
a kit or a component is checked against the host that will load it. Each kit's N-Triples
example runs through the same tests, so an extension gives the same quads whichever
language it was written in.

## Trying it

The `parse` example runs a document through a parser component and prints the quads the
host stored, or the failure with the line and column it points at. With the submodules
checked out, build the Turtle component, then parse one of the samples:

```shell
(cd components/turtle-parser && cargo build --release --target wasm32-wasip3)
cargo run --example parse -- --base https://pod.example/profile/card host/examples/profile.ttl
```

`host/examples/broken.ttl` shows a syntax error found by the component, and
`host/examples/undeclared-prefix.ttl` a statement refused by the host.

## Benchmarks

The `benchmarks` crate measures passing typed statements across the component boundary
against passing text, and an extension written in TypeScript against the same extension in
Rust. [`benchmarks/README.md`](benchmarks/README.md) describes the method and records the
results. Run it with `cargo bench -p benchmarks`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how changes are made and what is checked before
they land.

## Licence

MPL-2.0. Copyright (c) Omnifi Foundation.
