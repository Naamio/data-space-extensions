// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Resource Description Framework (RDF) terms and quads, built and checked from their
//! written forms.
//!
//! Each function here takes one written form and returns the matching [`oxrdf`] value, or a
//! [`Problem`] naming what is wrong. Positions are already enforced by the written types: a
//! subject can only be an Internationalized Resource Identifier (IRI) or a blank node, and
//! a predicate only an IRI. What remains to
//! check is the content of each term, and the references between a quad and its triple
//! terms.

use crate::blank_nodes::BlankNodeScope;
use crate::interpretation::{ErrorKind, Problem};
use crate::namespaces::Namespaces;
use crate::written::{Annotation, BaseDirection, Literal, Node, Quad, Term, Triple};

/// The datatype of a literal with a language and no base direction.
const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// The datatype of a literal with a language and a base direction.
const RDF_DIR_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#dirLangString";

/// Builds a quad, with its triple terms, from its written form.
pub fn quad(
	quad: &Quad,
	namespaces: &Namespaces,
	blank_nodes: &mut BlankNodeScope,
) -> Result<oxrdf::Quad, Problem> {
	let triple_terms = triple_terms(&quad.triple_terms, namespaces, blank_nodes)?;
	Ok(oxrdf::Quad::new(
		node(&quad.subject, namespaces, blank_nodes)?,
		namespaces.resolve(&quad.predicate)?,
		term(&quad.object, &triple_terms, namespaces, blank_nodes)?,
		match &quad.graph_name {
			None => oxrdf::GraphName::DefaultGraph,
			Some(graph_name) => match node(graph_name, namespaces, blank_nodes)? {
				oxrdf::NamedOrBlankNode::NamedNode(name) => oxrdf::GraphName::NamedNode(name),
				oxrdf::NamedOrBlankNode::BlankNode(name) => oxrdf::GraphName::BlankNode(name),
			},
		},
	))
}

/// Builds the triples of a quad's triple terms, in order.
///
/// Each entry may refer only to the entries already built, which is what makes the order
/// of the list matter and rules out cycles.
fn triple_terms(
	written: &[Triple],
	namespaces: &Namespaces,
	blank_nodes: &mut BlankNodeScope,
) -> Result<Vec<oxrdf::Triple>, Problem> {
	let mut built = Vec::with_capacity(written.len());
	for triple in written {
		let subject = node(&triple.subject, namespaces, blank_nodes)?;
		let predicate = namespaces.resolve(&triple.predicate)?;
		let object = term(&triple.object, &built, namespaces, blank_nodes)?;
		built.push(oxrdf::Triple::new(subject, predicate, object));
	}
	Ok(built)
}

/// Builds a subject or a graph name.
fn node(
	node: &Node,
	namespaces: &Namespaces,
	blank_nodes: &mut BlankNodeScope,
) -> Result<oxrdf::NamedOrBlankNode, Problem> {
	Ok(match node {
		Node::Iri(reference) => namespaces.resolve(reference)?.into(),
		Node::BlankNode(blank_node) => blank_nodes.resolve(blank_node).into(),
	})
}

/// Builds an object term. `available` holds the triple terms the term may refer to.
fn term(
	term: &Term,
	available: &[oxrdf::Triple],
	namespaces: &Namespaces,
	blank_nodes: &mut BlankNodeScope,
) -> Result<oxrdf::Term, Problem> {
	Ok(match term {
		Term::Iri(reference) => namespaces.resolve(reference)?.into(),
		Term::BlankNode(blank_node) => blank_nodes.resolve(blank_node).into(),
		Term::Literal(written) => literal(written, namespaces)?.into(),
		Term::TripleTerm(position) => {
			let triple = usize::try_from(*position)
				.ok()
				.and_then(|position| available.get(position))
				.ok_or_else(|| Problem {
					kind: ErrorKind::InvalidTripleTerm,
					message: format!(
						"the triple term refers to entry {position}, but only {} earlier entries exist",
						available.len()
					),
				})?;
			oxrdf::Term::Triple(Box::new(triple.clone()))
		}
	})
}

/// Builds a literal.
///
/// RDF gives every literal exactly one datatype. Without an annotation it is `xsd:string`.
/// A language makes it `rdf:langString`, or `rdf:dirLangString` with a base direction, so
/// those two datatypes cannot be written directly: a literal claiming one of them without a
/// language would have no language to carry.
fn literal(written: &Literal, namespaces: &Namespaces) -> Result<oxrdf::Literal, Problem> {
	let value = written.lexical_form.as_str();
	match &written.annotation {
		None => Ok(oxrdf::Literal::new_simple_literal(value)),
		Some(Annotation::Datatype(reference)) => {
			let datatype = namespaces.resolve(reference)?;
			if matches!(datatype.as_str(), RDF_LANG_STRING | RDF_DIR_LANG_STRING) {
				return Err(Problem {
					kind: ErrorKind::InvalidLiteral,
					message: format!(
						"the literal \"{value}\" names the datatype <{datatype}>, which requires a language"
					),
				});
			}
			Ok(oxrdf::Literal::new_typed_literal(value, datatype))
		}
		Some(Annotation::Language(language)) => {
			let built = language.direction.map_or_else(
				|| oxrdf::Literal::new_language_tagged_literal(value, &language.tag),
				|direction| {
					oxrdf::Literal::new_directional_language_tagged_literal(
						value,
						&language.tag,
						base_direction(direction),
					)
				},
			);
			built.map_err(|error| Problem {
				kind: ErrorKind::InvalidLiteral,
				message: format!(
					"the literal \"{value}\" has a malformed language tag `{}`: {error}",
					language.tag
				),
			})
		}
	}
}

/// The `oxrdf` form of a written base direction.
const fn base_direction(direction: BaseDirection) -> oxrdf::BaseDirection {
	match direction {
		BaseDirection::Ltr => oxrdf::BaseDirection::Ltr,
		BaseDirection::Rtl => oxrdf::BaseDirection::Rtl,
	}
}
