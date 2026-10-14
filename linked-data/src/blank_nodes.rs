// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Giving blank nodes their identity within one document.
//!
//! RDF 1.2 Concepts states that blank node identifiers are not part of the abstract data
//! model: a label in a document only says which mentions refer to the same node. The host
//! therefore creates its own node the first time a document mentions a label or a generated
//! number, and returns that node for every later mention in the same document.

use std::collections::HashMap;

use crate::written::BlankNode;

/// The blank nodes one document has mentioned so far.
///
/// Written labels and generated numbers are kept in separate maps, so a label that happens
/// to be the digits of a generated number still refers to a different node.
#[derive(Debug, Default)]
pub struct BlankNodeScope {
	/// The node for each label the document has written.
	written: HashMap<String, oxrdf::BlankNode>,
	/// The node for each number the parser has generated.
	generated: HashMap<u64, oxrdf::BlankNode>,
}

impl BlankNodeScope {
	/// The node a mention refers to, created if this is the first mention.
	///
	/// A new node takes a fresh random identifier from [`oxrdf::BlankNode::default`], not the
	/// document's label, so nodes from different documents never share an identifier.
	pub fn resolve(&mut self, blank_node: &BlankNode) -> oxrdf::BlankNode {
		match blank_node {
			BlankNode::Written(label) => self.written.entry(label.clone()).or_default().clone(),
			BlankNode::Generated(number) => self.generated.entry(*number).or_default().clone(),
		}
	}
}
