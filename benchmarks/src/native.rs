// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Parsing a document in the host, with no component.
//!
//! This is the floor the other paths are measured against, since it does the least work:
//! no component, no boundary, one parser that resolves as it reads. It is also the second
//! half of the text path, which parses the component's Turtle with [`read_turtle`].
//!
//! The parsers are oxttl's, from the same Oxigraph release as the oxrdf types the dataset
//! stores, so each parsed triple goes into the dataset without conversion and no time is
//! spent translating between libraries.

use std::error::Error;
use std::fmt;

use linked_data::dataset::Dataset;
use linked_data::oxrdf::GraphName;
use oxttl::{NTriplesParser, TurtleParser};

/// A failure to read a document into the dataset: a syntax error oxttl found, an invalid
/// base, or the dataset's limit on quads, each described by the message its source gave.
#[derive(Debug)]
pub struct NativeError(String);

impl fmt::Display for NativeError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.0)
	}
}

impl Error for NativeError {}

/// Keeps the message of an error from oxttl or the dataset. The three possible sources have
/// three different error types, and the benchmark only ever reports the message.
fn native_error(error: impl fmt::Display) -> NativeError {
	NativeError(error.to_string())
}

/// Parses the Turtle `document`, resolving relative references against `base`, and adds
/// its triples to the default graph of `dataset`.
///
/// Returns how many quads were new to the dataset. A triple stated twice is stored once and
/// counted once, so the count equals the number of distinct triples the document states.
///
/// # Errors
///
/// Returns the first syntax error, an invalid base, or the dataset's limit being reached.
pub fn read_turtle(
	document: &[u8],
	base: &str,
	dataset: &mut Dataset,
) -> Result<usize, NativeError> {
	let parser = TurtleParser::new()
		.with_base_iri(base)
		.map_err(native_error)?;
	let mut added = 0;
	for triple in parser.for_slice(document) {
		let quad = triple
			.map_err(native_error)?
			.in_graph(GraphName::DefaultGraph);
		added += usize::from(dataset.insert(&quad).map_err(native_error)?);
	}
	Ok(added)
}

/// Parses the N-Triples `document` and adds its triples to the default graph of `dataset`.
/// N-Triples writes every name in full, so there is no base to resolve against.
///
/// Returns how many quads were new to the dataset, counted as [`read_turtle`] counts them.
///
/// # Errors
///
/// Returns the first syntax error, or the dataset's limit being reached.
pub fn read_n_triples(document: &[u8], dataset: &mut Dataset) -> Result<usize, NativeError> {
	let mut added = 0;
	for triple in NTriplesParser::new().for_slice(document) {
		let quad = triple
			.map_err(native_error)?
			.in_graph(GraphName::DefaultGraph);
		added += usize::from(dataset.insert(&quad).map_err(native_error)?);
	}
	Ok(added)
}
