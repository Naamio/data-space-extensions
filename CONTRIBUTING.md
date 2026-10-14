# Contributing to Naamio Data Space Extensions

Start with the [README](README.md) for what this repository is and how it fits into Naamio
Data Space.

## Getting the source

The contract between the host and its components lives in its own repository, and so do
the Rust kit and the first-party components. Each is included here as a Git submodule:

- `specifications/wasi`, the contract
- `kits/rust`, the Rust kit and its example extensions
- `kits/javascript`, the JavaScript kit and its example extensions, written in TypeScript
- `components/turtle-parser`, the Turtle parser

Clone with them:

```shell
git clone --recurse-submodules git@github.com:Naamio/data-space-extensions.git
```

Do not edit anything inside a submodule. Change it in its own repository, then move the
submodule forward here in its own merge request.

The host is the only repository that includes the others. None of them may include the
host, or anything that includes it, because recursive clones and Cargo would both follow
the cycle.

## Tests first

Write the failing test, then the smallest code that passes it, then improve what is there if
improving it is worth doing.

A test documents behaviour and treats the implementation as a black box. Name it for what it
establishes, and say in its documentation why that matters, so someone changing the code
later knows what they would be breaking.

Do not write a type before a test asks for it. Interfaces invented ahead of a consumer are
the ones that turn out to be unimplementable. A documented, empty, compiling crate is
preferable to one carrying a type nothing yet exercises.

## Gates

```shell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The feature tests build the JavaScript kit's examples as well as the Rust components, so
they need Node 22.15 or later and pnpm on the `PATH`. Set `PNPM` to the pnpm executable if it
is somewhere else.

Clippy runs at `deny` for `all`, `pedantic`, and the lints named individually in
`[workspace.lints]`, with thresholds in `.clippy.toml`. Do not weaken a lint to make code
compile: change the code, or if the lint is genuinely wrong for a case, mark that one case
with `#[expect(...)]` and a `reason` explaining it. Use `expect` and not `allow`, so the
exemption is reported once it stops being needed.

Needing the same exemption in several places means the rule is scoped wrongly. Fix the
configuration instead of repeating the attribute.

Every `.rs` file and every `Cargo.toml` carries an SPDX header, in the comment syntax for
that file type:

```rust
// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0
```

## Boundaries

The sync and storage engine of Naamio Data Space handles only ciphertext, and must never
depend on a crate in this repository. A crate here may depend on the engine's public
interfaces, but nothing in the engine may depend on this repository. A change that would
make the engine depend on extensions needs an architecture record first.

## Comments

Public items are documented with their arguments, what they return, what they panic on, and
an example where one helps. Private items are documented too; `missing_docs_in_private_items`
is denied.

Write for someone reading English as an additional language, and for someone meeting the code
for the first time. Say what something does and why it is that way. Do not explain the build
configuration in the file, and do not write about the writing.

## Names

No abbreviations: `configuration`, not `config`. `misc`, `common`, `core`, `utils`, `shared`
and `types` are refused as crate or module names, each one a place a decision got deferred
rather than made. Crates carry no `naamio` prefix. "User" is refused too: name the person by
their role, such as *account holder*, *operator* or *developer*.

## Commits and merge requests

Commit messages are a single line, in the imperative, specific enough to identify the
change. Detail belongs in the merge request.

Keep unrelated changes apart. A rename, a formatting pass, and a behaviour change are three
merge requests, because reviewing them together means reviewing none of them properly.

## Versions

Check a version before writing it into a manifest. The toolchain is a nightly release
pinned by date in `rust-toolchain.toml`. It is the same release the components use for
`wasm32-wasip3`, so the host and its components are built with one compiler, and an
unpinned nightly would be a different compiler every day. Bump it deliberately, in its own
merge request, together with the components.
