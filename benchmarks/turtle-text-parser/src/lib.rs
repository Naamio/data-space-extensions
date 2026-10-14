// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The benchmark's component: the Turtle grammar, passing the statements a document makes
//! to the host in two forms.
//!
//! The component exports two interfaces. The contract's `parser` comes from the
//! `turtle-parser` crate, linked in whole, and streams typed statements. The benchmark's
//! `text-parser`, defined here, runs the same grammar and writes the same statements as
//! Turtle text. A benchmark that times both through this one binary is measuring the form
//! the statements cross the boundary in, and nothing else.
//!
//! [`writing`] turns statements into Turtle.

mod writing;

/// Rust bindings generated from the benchmark's `text-parser-component` world, and the
/// export of [`TextParser`] through them.
///
/// The crate's lints relax `unsafe_code` from `forbid` to `deny` for this module only.
/// wit-bindgen expands its export code in this crate, and that code uses `unsafe` to
/// implement the canonical ABI. The Rust kit avoids this for extensions by exporting
/// through a macro defined in its own crate, but the kit has no macro for a benchmark-only
/// world, and adding one would put a benchmark into the kit.
#[cfg_attr(
	target_family = "wasm",
	expect(
		unsafe_code,
		reason = "wit-bindgen's generated exports use `unsafe` to implement the canonical ABI"
	)
)]
#[expect(
	clippy::same_length_and_capacity,
	reason = "wit-bindgen's generated code rebuilds vectors passed across the boundary"
)]
mod bindings {
	wit_bindgen::generate!({
		path: "../wit",
		world: "text-parser-component",
	});

	// The export defines symbols named after the interface, which only a WebAssembly link
	// can produce, and the macro accepts a type name but not a path.
	#[cfg(target_family = "wasm")]
	use super::TextParser;
	#[cfg(target_family = "wasm")]
	export!(TextParser with_types_in self);
}

use bindings::exports::naamio::benchmarks::text_parser::Guest;
use bindings::{wit_future, wit_stream};
use wit_bindgen::{FutureReader, StreamReader};

/// The component's implementation of the benchmark's `text-parser` interface.
#[derive(Debug)]
struct TextParser;

impl Guest for TextParser {
	/// Returns the text stream and the outcome future at once, then reads the document,
	/// runs the Turtle grammar over it, and writes the statements as Turtle.
	///
	/// The work runs in a task started with `spawn_local`, after this function returns, as
	/// it does in the Rust kit's exports, so both paths return their streams and do their
	/// work in the same way. The text is written in one batch, as the typed path's
	/// statements are.
	#[expect(
		clippy::unused_async_trait_impl,
		reason = "the interface requires an asynchronous export; the waiting happens in the spawned task"
	)]
	async fn parse(
		document: StreamReader<u8>,
	) -> (StreamReader<u8>, FutureReader<Result<(), String>>) {
		let (mut text, text_reader) = wit_stream::new::<u8>();
		let (outcome, outcome_reader) = wit_future::new::<Result<(), String>>(|| Ok(()));

		wit_bindgen::spawn_local(async move {
			let bytes = document.collect().await;
			let reading = turtle_parser::grammar::read(&bytes);
			let written = writing::turtle(&reading.statements);
			// The host may stop reading early; the bytes it did not take have no reader.
			let _ = text.write_all(written).await;
			drop(text);
			let result = reading
				.outcome
				.map_err(|error| format!("the grammar refused the document: {error:?}"));
			// The host may have stopped listening, in which case there is no one to tell.
			let _ = outcome.write(result).await;
		});

		(text_reader, outcome_reader)
	}
}
