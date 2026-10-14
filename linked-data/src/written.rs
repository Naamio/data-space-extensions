// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Statements as a document writes them.
//!
//! These types follow the `terms` interface of the `linked-data:rdf` contract field for
//! field, with no WebAssembly bindings, so that interpretation can be used and tested
//! without a component. Nothing here is checked yet: a reference may be relative, a prefix
//! may be undeclared, and a language tag may be malformed. The
//! [`Interpreter`](crate::interpretation::Interpreter) decides what each statement means,
//! and whether it is valid, when it receives it.

/// A reference to an Internationalized Resource Identifier (IRI), as a document writes it.
/// Escape sequences are already decoded in every form.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IriReference {
	/// An absolute IRI, to be used exactly as written, without resolution.
	Absolute(String),
	/// A relative reference, to be resolved against the base in effect where it appears.
	Relative(String),
	/// A prefixed name such as `foaf:name`, to be expanded against the prefix declarations
	/// that precede it.
	Prefixed {
		/// The prefix, without the colon. The empty string is the default prefix.
		prefix: String,
		/// The local part.
		local: String,
	},
}

/// A blank node, as a document refers to it. Both forms are scoped to one document, and a
/// written label never refers to the same node as a generated number.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlankNode {
	/// A label the document writes, such as `b0` for Turtle's `_:b0`.
	Written(String),
	/// A number the parser assigns to a node the document does not label, such as Turtle's
	/// `[ ]`.
	Generated(u64),
}

/// A node that can be the subject of a triple or the name of a graph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Node {
	/// An IRI.
	Iri(IriReference),
	/// A blank node.
	BlankNode(BlankNode),
}

/// The base direction of the text in a literal, as RDF 1.2 defines it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BaseDirection {
	/// Left to right.
	Ltr,
	/// Right to left.
	Rtl,
}

/// A language tag with an optional base direction.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Language {
	/// The language tag, as defined by BCP 47.
	pub tag: String,
	/// The base direction of the text, if the document gives one.
	pub direction: Option<BaseDirection>,
}

/// A literal's datatype or language, never both: the part of a literal beyond its text.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Annotation {
	/// The datatype.
	Datatype(IriReference),
	/// The language, and optionally the base direction.
	Language(Language),
}

/// A literal, as a document writes it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Literal {
	/// The lexical form, with escape sequences decoded.
	pub lexical_form: String,
	/// The datatype or the language. With neither, the literal is an `xsd:string`.
	pub annotation: Option<Annotation>,
}

/// A term in the object position of a triple.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Term {
	/// An IRI.
	Iri(IriReference),
	/// A blank node.
	BlankNode(BlankNode),
	/// A literal.
	Literal(Literal),
	/// A triple term, given as a position in the containing quad's `triple_terms`, counting
	/// from zero.
	TripleTerm(u32),
}

/// The triple a triple term stands for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Triple {
	/// The subject.
	pub subject: Node,
	/// The predicate.
	pub predicate: IriReference,
	/// The object.
	pub object: Term,
}

/// A triple in a dataset, with the graph it belongs to and the triple terms it uses.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Quad {
	/// The subject.
	pub subject: Node,
	/// The predicate.
	pub predicate: IriReference,
	/// The object.
	pub object: Term,
	/// The name of the graph the triple belongs to, or `None` for the default graph.
	pub graph_name: Option<Node>,
	/// The triples of the triple terms this quad uses. An entry may refer only to entries
	/// before it.
	pub triple_terms: Vec<Triple>,
}

/// A range of bytes in a document's encoded form, counting from zero. The range includes
/// `start` and excludes `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
	/// The offset of the first byte in the range.
	pub start: u64,
	/// The offset of the first byte after the range.
	pub end: u64,
}

/// One thing a document says, in the order it says it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Statement {
	/// A triple in a dataset.
	Quad(Quad),
	/// A prefix declaration.
	Prefix {
		/// The prefix, without the colon.
		prefix: String,
		/// The namespace the prefix expands to.
		namespace: IriReference,
	},
	/// A base declaration, which changes the base for the statements after it.
	Base(IriReference),
	/// The version of the syntax the document claims to follow.
	Version(String),
	/// Something the parser accepted but a person should know about, described in English.
	Warning(String),
}

/// A statement, with the part of the document it comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocatedStatement {
	/// The bytes of the document the statement comes from.
	pub span: Span,
	/// The statement.
	pub statement: Statement,
}
