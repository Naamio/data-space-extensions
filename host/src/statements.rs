// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Moving statements from the contract's generated types into `linked-data`'s written
//! forms.
//!
//! The two models describe the same thing field for field. They are kept apart so that
//! `linked-data` has no dependency on Wasmtime and can interpret statements from any
//! source, including a host in another language that loads the interpreter as a component.
//! Nothing is checked on the way across: an invalid Internationalized Resource Identifier
//! (IRI), an undeclared prefix or a dangling triple term passes through unchanged, and the
//! interpreter reports it with the statement's span.

use linked_data::written;

use crate::bindings::linked_data::rdf::terms;

/// The statement a component streamed, with the span it gave, in `linked-data`'s form.
/// This is the one entry point the parse loop needs; everything below serves it.
pub fn located_statement(located: terms::LocatedStatement) -> written::LocatedStatement {
	written::LocatedStatement {
		span: span(located.span),
		statement: statement(located.statement),
	}
}

/// The span a component reported. Spans are byte offsets in the document as the component
/// received it, so they pass through untouched; the host turns them into lines and columns
/// only when it reports a problem.
pub const fn span(span: terms::Span) -> written::Span {
	written::Span {
		start: span.start,
		end: span.end,
	}
}

/// The statement, by kind. The contract wraps a prefix declaration in a record of its own,
/// where `linked-data` holds the prefix and namespace on the variant directly; the other
/// kinds carry the same payload in both models.
fn statement(statement: terms::Statement) -> written::Statement {
	match statement {
		terms::Statement::Quad(written_quad) => written::Statement::Quad(quad(written_quad)),
		terms::Statement::Prefix(declaration) => written::Statement::Prefix {
			prefix: declaration.prefix,
			namespace: iri_reference(declaration.namespace),
		},
		terms::Statement::Base(reference) => written::Statement::Base(iri_reference(reference)),
		terms::Statement::Version(version) => written::Statement::Version(version),
		terms::Statement::Warning(message) => written::Statement::Warning(message),
	}
}

/// The quad, together with the list of triples its triple terms point into. The list keeps
/// its order, because a triple term refers to an entry by position.
fn quad(quad: terms::Quad) -> written::Quad {
	written::Quad {
		subject: node(quad.subject),
		predicate: iri_reference(quad.predicate),
		object: term(quad.object),
		graph_name: quad.graph_name.map(node),
		triple_terms: quad.triple_terms.into_iter().map(triple).collect(),
	}
}

/// One entry of a quad's triple-term list.
fn triple(triple: terms::Triple) -> written::Triple {
	written::Triple {
		subject: node(triple.subject),
		predicate: iri_reference(triple.predicate),
		object: term(triple.object),
	}
}

/// A subject or a graph name, which both models restrict to an IRI or a blank node.
fn node(node: terms::Node) -> written::Node {
	match node {
		terms::Node::Iri(reference) => written::Node::Iri(iri_reference(reference)),
		terms::Node::BlankNode(blank) => written::Node::BlankNode(blank_node(blank)),
	}
}

/// An object term. A triple term's position is carried across as a number; whether it
/// points at an earlier entry of the list is for the interpreter to decide.
fn term(term: terms::Term) -> written::Term {
	match term {
		terms::Term::Iri(reference) => written::Term::Iri(iri_reference(reference)),
		terms::Term::BlankNode(blank) => written::Term::BlankNode(blank_node(blank)),
		terms::Term::Literal(written_literal) => written::Term::Literal(literal(written_literal)),
		terms::Term::TripleTerm(position) => written::Term::TripleTerm(position),
	}
}

/// An IRI reference in the form the document wrote it. The form matters to the
/// interpreter, which uses an absolute IRI verbatim, resolves a relative one, and expands a
/// prefixed one, so the three are never merged here.
fn iri_reference(reference: terms::IriReference) -> written::IriReference {
	match reference {
		terms::IriReference::Absolute(iri) => written::IriReference::Absolute(iri),
		terms::IriReference::Relative(relative) => written::IriReference::Relative(relative),
		terms::IriReference::Prefixed(name) => written::IriReference::Prefixed {
			prefix: name.prefix,
			local: name.local,
		},
	}
}

/// A blank node, still in its written or generated form. The interpreter keeps the two
/// forms in separate scopes, so the translation must keep them distinct: merging them would
/// join a document's label `1` and the parser's generated node 1 into one node.
fn blank_node(blank_node: terms::BlankNode) -> written::BlankNode {
	match blank_node {
		terms::BlankNode::Written(label) => written::BlankNode::Written(label),
		terms::BlankNode::Generated(number) => written::BlankNode::Generated(number),
	}
}

/// A literal, with its lexical form untouched and its annotation, a datatype or a language
/// with an optional base direction, carried in the same shape.
fn literal(literal: terms::Literal) -> written::Literal {
	written::Literal {
		lexical_form: literal.lexical_form,
		annotation: literal.annotation.map(annotation),
	}
}

/// A literal's datatype or language. The contract's variant already rules out a literal
/// with both, so there is nothing to reconcile.
fn annotation(annotation: terms::Annotation) -> written::Annotation {
	match annotation {
		terms::Annotation::Datatype(reference) => {
			written::Annotation::Datatype(iri_reference(reference))
		}
		terms::Annotation::Language(language) => written::Annotation::Language(written::Language {
			tag: language.tag,
			direction: language.direction.map(base_direction),
		}),
	}
}

/// The base direction that Resource Description Framework (RDF) 1.2 gives a
/// language-tagged literal: left to right or right to left.
const fn base_direction(direction: terms::BaseDirection) -> written::BaseDirection {
	match direction {
		terms::BaseDirection::Ltr => written::BaseDirection::Ltr,
		terms::BaseDirection::Rtl => written::BaseDirection::Rtl,
	}
}
