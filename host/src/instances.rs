// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The state the host keeps for each running instance of a component.

use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use crate::bindings::linked_data::rdf::{syntax, terms};

/// The data a Wasmtime store holds for one instance of a component.
///
/// The WebAssembly System Interface (WASI) context is built empty. The instance sees no
/// environment variables, no command-line arguments, no files, no network, and no standard
/// input or output. A component built with the Rust standard library, or with the QuickJS
/// engine for TypeScript, imports WASI interfaces whether or not it uses them, so the host
/// links them, but grants nothing through them.
pub struct InstanceState {
	/// The WASI context, built with no capabilities.
	wasi: WasiCtx,
	/// The resources the instance holds handles to, including the streams and futures of a
	/// parse in progress.
	table: ResourceTable,
}

impl InstanceState {
	/// Creates the state for a new instance, with an empty WASI context and an empty
	/// resource table.
	pub fn new() -> Self {
		Self {
			wasi: WasiCtx::builder().build(),
			table: ResourceTable::new(),
		}
	}
}

/// Gives Wasmtime's WASI implementation the context and resource table it works on.
impl WasiView for InstanceState {
	fn ctx(&mut self) -> WasiCtxView<'_> {
		WasiCtxView {
			ctx: &mut self.wasi,
			table: &mut self.table,
		}
	}
}

/// The `terms` interface declares types and no functions. A component that uses its types
/// imports it all the same, so the host provides it, with nothing to implement.
impl terms::Host for InstanceState {}

/// The `syntax` interface declares types and no functions, and is provided for the same
/// reason as `terms`.
impl syntax::Host for InstanceState {}
