// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The WebAssembly engine and the linker that every component runs on.

use std::error::Error;
use std::fmt;

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine};

use crate::bindings::ParserComponentPre;
use crate::instances::InstanceState;
use crate::parsers::ParserComponent;

/// A failure in running a component, as distinct from a problem in a document.
///
/// The component may not be valid WebAssembly, may import an interface the host does not
/// provide, may lack the export the host expected, or may trap while it runs. The message
/// includes the chain of causes Wasmtime reports.
#[derive(Debug)]
pub struct ComponentError(wasmtime::Error);

impl ComponentError {
	/// A failure the host itself detects, such as a component that ends a parse without
	/// reporting how it ended.
	pub(crate) fn host(message: &str) -> Self {
		Self(wasmtime::Error::msg(message.to_owned()))
	}
}

impl fmt::Display for ComponentError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(formatter, "the component failed: {:#}", self.0)
	}
}

impl Error for ComponentError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		Some(self.0.as_ref())
	}
}

impl From<wasmtime::Error> for ComponentError {
	fn from(error: wasmtime::Error) -> Self {
		Self(error)
	}
}

/// The extension host.
///
/// A host holds one Wasmtime engine and one linker. The linker provides WebAssembly System
/// Interface (WASI) 0.3 and 0.2, with no capabilities granted through either, and the
/// type-only interfaces of the `linked-data:rdf` contract. Every component the host loads is linked
/// against it. Compiling a component is the slow step, so a host is created once and used
/// to load every component.
pub struct ExtensionHost {
	/// The engine that compiles and runs components.
	engine: Engine,
	/// The interfaces every component is linked against.
	linker: Linker<InstanceState>,
}

/// Prints the type name only; the engine and linker hold nothing a reader of a debug
/// message could use.
impl fmt::Debug for ExtensionHost {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("ExtensionHost")
			.finish_non_exhaustive()
	}
}

impl ExtensionHost {
	/// Creates a host.
	///
	/// The engine has the component model's asynchronous features turned on, which the
	/// streams and futures of WASI 0.3 depend on.
	///
	/// # Errors
	///
	/// Returns an error if Wasmtime cannot create an engine with this configuration on the
	/// current platform, or if adding the host's interfaces to the linker fails.
	pub fn new() -> Result<Self, ComponentError> {
		let mut configuration = Config::new();
		configuration.wasm_component_model_async(true);
		let engine = Engine::new(&configuration)?;

		let mut linker = Linker::new(&engine);
		// Components written in TypeScript are built with componentize-qjs, whose QuickJS
		// engine imports the WASI 0.2 interfaces as well as the contract's WASI 0.3 streams.
		wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
		wasmtime_wasi::p3::add_to_linker(&mut linker)?;
		crate::bindings::ParserComponent::add_to_linker::<InstanceState, HasSelf<InstanceState>>(
			&mut linker,
			|state| state,
		)?;

		Ok(Self { engine, linker })
	}

	/// Compiles a component that exports the `parser` interface, and links it against the
	/// host's interfaces.
	///
	/// Linking happens here, before the component is given a document. A component that
	/// imports something the host does not provide is refused at this point, and the error
	/// names the import.
	///
	/// # Errors
	///
	/// Returns an error if `component` is not a valid WebAssembly component, imports an
	/// interface the host does not provide, or does not export the `parser` interface.
	pub fn load_parser(&self, component: &[u8]) -> Result<ParserComponent, ComponentError> {
		let component = Component::new(&self.engine, component)?;
		let prepared = ParserComponentPre::new(self.linker.instantiate_pre(&component)?)?;

		Ok(ParserComponent::new(self.engine.clone(), prepared))
	}
}
