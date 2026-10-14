// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Rust bindings generated from the `parser-component` world of the `linked-data:rdf`
//! contract.
//!
//! Wasmtime generates these when the crate compiles, from the WebAssembly Interface Type
//! (WIT) files in the `specifications/wasi` submodule. If the contract changes in a way the
//! host does not match, the host stops compiling.
//!
//! Exports are called concurrently, from inside the store's event loop. A parser returns
//! its statement stream and outcome future at once and keeps writing to them afterwards,
//! from a task it has spawned. That task makes progress only while the store's event loop
//! runs, so the call, the reading of the stream, and the wait for the outcome all happen
//! inside one `run_concurrent`, which takes the `Accessor` these calls require.

wasmtime::component::bindgen!({
	path: "../specifications/wasi/rdf",
	world: "parser-component",
	exports: { default: async | store },
});
