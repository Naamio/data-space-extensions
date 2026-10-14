// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The Resource Description Framework (RDF) data model, owned by the extension host.
//!
//! A parser component reports what a document says, as written: Internationalized Resource
//! Identifiers (IRIs) that may be relative or prefixed, blank nodes referred to by labels,
//! and literals as text with a datatype or a language. This crate does everything common to
//! every syntax. It interprets those statements into RDF quads, checking every term, and
//! stores the quads in a dataset.
//!
//! The representation is [`oxrdf`], re-exported here so that code reading the quads uses
//! the same version of it as this crate does.
//!
//! - [`written`] describes statements as a document writes them.
//! - [`interpretation`] turns those statements into quads.
//! - [`dataset`] stores the quads, with an optional limit.
//! - [`positions`] turns the byte offsets parsers report into lines and columns.

mod blank_nodes;
pub mod dataset;
pub mod interpretation;
mod namespaces;
pub mod positions;
mod terms;
pub mod written;

pub use oxrdf;
