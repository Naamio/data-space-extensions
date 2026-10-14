// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The Naamio Data Space extension host.
//!
//! The host runs WebAssembly components built for WebAssembly System Interface (WASI) 0.3
//! that implement the `linked-data:rdf` contract. A parser component knows one syntax: it
//! reads a document and streams back the statements the document makes, as written. The
//! host owns their meaning. It interprets each statement with the `linked-data` crate and
//! stores the resulting Resource Description Framework (RDF) quads in a dataset.
//!
//! - [`runtime`] holds the engine and linker every component runs on.
//! - [`parsers`] loads parser components and runs a parse.
//!
//! The other modules are internal: the generated bindings, the state of each running
//! instance, the translation of statements into `linked-data`'s types, the ingestion of
//! statements into a dataset, and the host's ends of a component's streams.

mod bindings;
mod ingestion;
mod instances;
pub mod parsers;
pub mod runtime;
mod statements;
mod streams;
