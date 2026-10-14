// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Measures passing typed statements across the component boundary against passing text.
//!
//! The `linked-data:rdf` contract rests on the claim that a parser component streaming
//! typed statements to the host is faster than one passing text for the host to parse again. This crate holds what the
//! benchmark needs to test that claim:
//!
//! - [`documents`] generates Turtle and N-Triples documents of any size.
//! - [`text`] runs the text path: a component writes the statements as Turtle, and the
//!   host parses that text.
//! - [`native`] parses a document in the host, with no component, as the floor.
//!
//! The typed path is the extension host itself. The timings are in `benches/ingestion.rs`.

pub mod documents;
pub mod native;
pub mod text;
