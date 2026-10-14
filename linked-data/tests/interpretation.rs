// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests for interpreting the statements a parser writes.
//!
//! A parser component reports what a document says, as written. The interpreter does the
//! rest: it resolves Internationalized Resource Identifiers (IRIs), expands prefixes, gives
//! each blank node its identity, builds triple terms and literals, and checks every term.
//! These tests feed the interpreter statements directly, without a component, and check
//! the Resource Description Framework (RDF) data that comes out. The rules they hold the
//! interpreter to are the ones the `linked-data:rdf` contract promises parser authors.

use std::error::Error;

use linked_data::interpretation::{ErrorKind, Interpretation, InterpretationError, Interpreter};
use linked_data::oxrdf;
use linked_data::written::{
	Annotation, BaseDirection, BlankNode, IriReference, Language, Literal, LocatedStatement, Node,
	Quad, Span, Statement, Term, Triple,
};

/// A boxed error type broad enough for anything a test below might fail on.
type TestResult = Result<(), Box<dyn Error>>;

/// The datatype RDF gives a literal written with neither a datatype nor a language.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// A datatype the tests write through a prefix, to check that datatypes expand like any
/// other IRI reference.
const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

/// The datatype of a literal with a language and no base direction.
const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// The datatype RDF 1.2 gives a literal with a language and a base direction.
const RDF_DIR_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#dirLangString";

/// Builds a reference the interpreter must use exactly as written, without resolving it.
fn absolute(iri: &str) -> IriReference {
	IriReference::Absolute(iri.to_owned())
}

/// Builds a reference the interpreter resolves against the base in effect where it appears.
fn relative(reference: &str) -> IriReference {
	IriReference::Relative(reference.to_owned())
}

/// Builds a prefixed name such as `ex:s`, which the interpreter expands against the prefix
/// declarations that came before it.
fn prefixed(prefix: &str, local: &str) -> IriReference {
	IriReference::Prefixed {
		prefix: prefix.to_owned(),
		local: local.to_owned(),
	}
}

/// Builds a literal with neither a datatype nor a language, which RDF treats as an
/// `xsd:string`.
fn plain(lexical_form: &str) -> Term {
	Term::Literal(Literal {
		lexical_form: lexical_form.to_owned(),
		annotation: None,
	})
}

/// Builds a quad statement in the default graph with no triple terms, which is the shape of
/// an ordinary Turtle triple.
const fn quad(subject: Node, predicate: IriReference, object: Term) -> Statement {
	Statement::Quad(Quad {
		subject,
		predicate,
		object,
		graph_name: None,
		triple_terms: Vec::new(),
	})
}

/// Builds a quad with a fixed absolute subject and predicate, for tests that are only about
/// the object.
fn about_s(object: Term) -> Statement {
	quad(
		Node::Iri(absolute("https://example.org/s")),
		absolute("https://example.org/p"),
		object,
	)
}

/// Places a statement at bytes `start..end` of its document, for tests that check the span
/// an error or warning carries back.
const fn at(start: u64, end: u64, statement: Statement) -> LocatedStatement {
	LocatedStatement {
		span: Span { start, end },
		statement,
	}
}

/// Places a statement at the start of its document, for tests that do not check spans.
const fn anywhere(statement: Statement) -> LocatedStatement {
	at(0, 0, statement)
}

/// Interprets a statement that should produce a quad, and returns the quad. Anything else
/// becomes an error that says what came back instead.
fn expect_quad(
	interpreter: &mut Interpreter,
	statement: Statement,
) -> Result<oxrdf::Quad, Box<dyn Error>> {
	match interpreter.interpret(&anywhere(statement))? {
		Interpretation::Quad(quad) => Ok(quad),
		other => Err(format!("expected a quad, but the interpretation was {other:?}").into()),
	}
}

/// Interprets a statement that should be refused, and returns the refusal. A statement that
/// is accepted becomes an error that says what it produced.
fn expect_error(
	interpreter: &mut Interpreter,
	statement: &LocatedStatement,
) -> Result<InterpretationError, Box<dyn Error>> {
	match interpreter.interpret(statement) {
		Err(error) => Ok(error),
		Ok(other) => Err(format!("expected an error, but the interpretation was {other:?}").into()),
	}
}

/// Returns the quad's subject as an IRI, or an error if the subject is a blank node.
fn subject_iri(quad: &oxrdf::Quad) -> Result<&str, Box<dyn Error>> {
	match &quad.subject {
		oxrdf::NamedOrBlankNode::NamedNode(node) => Ok(node.as_str()),
		oxrdf::NamedOrBlankNode::BlankNode(node) => {
			Err(format!("expected an IRI subject, but it is {node}").into())
		}
	}
}

/// Returns the quad's subject as a blank node, or an error if the subject is an IRI.
fn subject_blank_node(quad: &oxrdf::Quad) -> Result<oxrdf::BlankNode, Box<dyn Error>> {
	match &quad.subject {
		oxrdf::NamedOrBlankNode::BlankNode(node) => Ok(node.clone()),
		oxrdf::NamedOrBlankNode::NamedNode(node) => {
			Err(format!("expected a blank node subject, but it is {node}").into())
		}
	}
}

/// Returns the quad's object as a literal, or an error if it is another kind of term.
fn object_literal(quad: &oxrdf::Quad) -> Result<oxrdf::Literal, Box<dyn Error>> {
	match &quad.object {
		oxrdf::Term::Literal(literal) => Ok(literal.clone()),
		other => Err(format!("expected a literal object, but it is {other}").into()),
	}
}

/// Returns a triple term's object as a literal, or an error if it is another kind of term.
fn triple_object_literal(triple: &oxrdf::Triple) -> Result<oxrdf::Literal, Box<dyn Error>> {
	match &triple.object {
		oxrdf::Term::Literal(literal) => Ok(literal.clone()),
		other => Err(format!("expected a literal object, but it is {other}").into()),
	}
}

/// A document fetched from a pod writes IRIs relative to where it was fetched from. The
/// host passes that address as the starting base, and relative references resolve against
/// it.
#[test]
fn resolves_a_relative_reference_against_the_starting_base() -> TestResult {
	let mut interpreter = Interpreter::new(Some("https://pod.example/people/"))?;

	let quad = expect_quad(
		&mut interpreter,
		quad(
			Node::Iri(relative("alice")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;

	assert_eq!(subject_iri(&quad)?, "https://pod.example/people/alice");

	Ok(())
}

/// A base declaration changes the base for the statements after it. A relative base is
/// itself resolved against the base in effect before it, as RFC 3986 requires.
#[test]
fn applies_a_base_declaration_to_the_statements_after_it() -> TestResult {
	let mut interpreter = Interpreter::new(Some("https://pod.example/people/"))?;

	interpreter.interpret(&anywhere(Statement::Base(relative("../things/"))))?;
	let quad = expect_quad(
		&mut interpreter,
		quad(
			Node::Iri(relative("lamp")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;

	assert_eq!(subject_iri(&quad)?, "https://pod.example/things/lamp");

	Ok(())
}

/// An absolute IRI is used exactly as written. N-Triples IRIs, and IRIs a JSON-LD parser
/// has already resolved, would change if the host resolved them again, because resolution
/// removes dot segments.
#[test]
fn uses_an_absolute_iri_exactly_as_written() -> TestResult {
	let mut interpreter = Interpreter::new(Some("https://pod.example/"))?;

	let quad = expect_quad(
		&mut interpreter,
		quad(
			Node::Iri(absolute("http://a.example/b/../c")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;

	assert_eq!(subject_iri(&quad)?, "http://a.example/b/../c");

	Ok(())
}

/// Without a base, a relative reference cannot be resolved. The error names the problem
/// and carries the statement's span, so the host can tell a person where to look.
#[test]
fn refuses_a_relative_reference_when_there_is_no_base() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;
	let statement = at(
		14,
		21,
		quad(
			Node::Iri(relative("alice")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	);

	let error = expect_error(&mut interpreter, &statement)?;

	assert_eq!(error.kind, ErrorKind::MissingBase);
	assert_eq!(error.span, Span { start: 14, end: 21 });

	Ok(())
}

/// A prefixed name expands to the namespace declared for its prefix, followed by its local
/// part. A later declaration of the same prefix applies to the statements after it, as
/// Turtle allows.
#[test]
fn expands_prefixed_names_using_the_latest_declaration() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;

	interpreter.interpret(&anywhere(Statement::Prefix {
		prefix: "ex".to_owned(),
		namespace: absolute("https://example.org/first/"),
	}))?;
	let first = expect_quad(
		&mut interpreter,
		quad(
			Node::Iri(prefixed("ex", "s")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;
	interpreter.interpret(&anywhere(Statement::Prefix {
		prefix: "ex".to_owned(),
		namespace: absolute("https://example.org/second/"),
	}))?;
	let second = expect_quad(
		&mut interpreter,
		quad(
			Node::Iri(prefixed("ex", "s")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;

	assert_eq!(subject_iri(&first)?, "https://example.org/first/s");
	assert_eq!(subject_iri(&second)?, "https://example.org/second/s");

	Ok(())
}

/// A prefix declared with a relative namespace takes the base in effect at the declaration,
/// not the base in effect when the prefix is used.
#[test]
fn resolves_a_relative_namespace_where_it_is_declared() -> TestResult {
	let mut interpreter = Interpreter::new(Some("https://pod.example/a/"))?;

	interpreter.interpret(&anywhere(Statement::Prefix {
		prefix: "local".to_owned(),
		namespace: relative("vocabulary#"),
	}))?;
	interpreter.interpret(&anywhere(Statement::Base(absolute(
		"https://elsewhere.example/",
	))))?;
	let quad = expect_quad(
		&mut interpreter,
		quad(
			Node::Iri(prefixed("local", "term")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;

	assert_eq!(subject_iri(&quad)?, "https://pod.example/a/vocabulary#term");

	Ok(())
}

/// A prefixed name whose prefix was never declared has no meaning. The error carries the
/// statement's span.
#[test]
fn refuses_a_prefix_that_was_never_declared() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;
	let statement = at(
		3,
		9,
		quad(
			Node::Iri(prefixed("ex", "s")),
			absolute("https://example.org/p"),
			plain("o"),
		),
	);

	let error = expect_error(&mut interpreter, &statement)?;

	assert_eq!(error.kind, ErrorKind::UndefinedPrefix);
	assert_eq!(error.span, Span { start: 3, end: 9 });

	Ok(())
}

/// The interpreter checks every absolute IRI rather than trusting the parser that sent it,
/// because the data it stores must be valid RDF whichever component produced it.
#[test]
fn refuses_an_iri_that_is_not_valid() -> TestResult {
	let cases = [
		(
			"an absolute IRI with a space",
			absolute("https://example.org/a b"),
		),
		("an absolute IRI with no scheme", absolute("example.org/a")),
	];

	for (case, subject) in cases {
		let mut interpreter = Interpreter::new(None)?;
		let statement = anywhere(quad(
			Node::Iri(subject),
			absolute("https://example.org/p"),
			plain("o"),
		));

		let error = expect_error(&mut interpreter, &statement)?;

		assert_eq!(
			error.kind,
			ErrorKind::InvalidIri,
			"{case} should be refused as an invalid IRI"
		);
	}

	Ok(())
}

/// Within one document, a written label refers to one node everywhere it appears. A second
/// document is interpreted by a second interpreter, and the same label there refers to a
/// different node, because RDF scopes blank node labels to their document.
#[test]
fn scopes_written_blank_node_labels_to_one_document() -> TestResult {
	let mut first_document = Interpreter::new(None)?;
	let mut second_document = Interpreter::new(None)?;
	let labelled = || {
		quad(
			Node::BlankNode(BlankNode::Written("b0".to_owned())),
			absolute("https://example.org/p"),
			plain("o"),
		)
	};

	let first = expect_quad(&mut first_document, labelled())?;
	let repeated = expect_quad(&mut first_document, labelled())?;
	let other = expect_quad(&mut second_document, labelled())?;

	assert_eq!(subject_blank_node(&first)?, subject_blank_node(&repeated)?);
	assert_ne!(subject_blank_node(&first)?, subject_blank_node(&other)?);

	Ok(())
}

/// A parser numbers the blank nodes it creates for `[ ]` and collections. A generated
/// number never refers to the same node as a written label, even when the label is the
/// number's digits, because the document's author chose the label and the parser chose the
/// number.
#[test]
fn keeps_generated_blank_nodes_apart_from_written_labels() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;

	let written = expect_quad(
		&mut interpreter,
		quad(
			Node::BlankNode(BlankNode::Written("1".to_owned())),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;
	let generated = expect_quad(
		&mut interpreter,
		quad(
			Node::BlankNode(BlankNode::Generated(1)),
			absolute("https://example.org/p"),
			plain("o"),
		),
	)?;

	assert_ne!(
		subject_blank_node(&written)?,
		subject_blank_node(&generated)?
	);

	Ok(())
}

/// RDF gives a literal with no datatype and no language the datatype `xsd:string`, so a
/// parser can leave the default out.
#[test]
fn gives_a_plain_literal_the_string_datatype() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;

	let quad = expect_quad(&mut interpreter, about_s(plain("Alice")))?;

	assert_eq!(object_literal(&quad)?.datatype().as_str(), XSD_STRING);

	Ok(())
}

/// A datatype is an IRI reference like any other, so `"42"^^xsd:integer` expands its
/// prefix before the literal is built.
#[test]
fn expands_a_prefixed_datatype() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;
	interpreter.interpret(&anywhere(Statement::Prefix {
		prefix: "xsd".to_owned(),
		namespace: absolute("http://www.w3.org/2001/XMLSchema#"),
	}))?;

	let quad = expect_quad(
		&mut interpreter,
		about_s(Term::Literal(Literal {
			lexical_form: "42".to_owned(),
			annotation: Some(Annotation::Datatype(prefixed("xsd", "integer"))),
		})),
	)?;

	assert_eq!(object_literal(&quad)?.datatype().as_str(), XSD_INTEGER);

	Ok(())
}

/// A language makes a literal an `rdf:langString`. With a base direction as well, RDF 1.2
/// makes it an `rdf:dirLangString` that keeps the direction.
#[test]
fn builds_language_and_directional_literals() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;
	let tagged = |direction| {
		about_s(Term::Literal(Literal {
			lexical_form: "שלום".to_owned(),
			annotation: Some(Annotation::Language(Language {
				tag: "he".to_owned(),
				direction,
			})),
		}))
	};

	let language_only = object_literal(&expect_quad(&mut interpreter, tagged(None))?)?;
	let directional = object_literal(&expect_quad(
		&mut interpreter,
		tagged(Some(BaseDirection::Rtl)),
	)?)?;

	assert_eq!(language_only.datatype().as_str(), RDF_LANG_STRING);
	assert_eq!(language_only.language(), Some("he"));
	assert_eq!(directional.datatype().as_str(), RDF_DIR_LANG_STRING);
	assert_eq!(directional.direction(), Some(oxrdf::BaseDirection::Rtl));

	Ok(())
}

/// Literals the RDF data model does not allow are refused as invalid literals: a malformed
/// language tag, and a datatype that requires a language, written without one.
#[test]
fn refuses_a_literal_rdf_does_not_allow() -> TestResult {
	let cases = [
		(
			"a malformed language tag",
			Annotation::Language(Language {
				tag: "not a tag".to_owned(),
				direction: None,
			}),
		),
		(
			"rdf:langString written as a datatype",
			Annotation::Datatype(absolute(RDF_LANG_STRING)),
		),
		(
			"rdf:dirLangString written as a datatype",
			Annotation::Datatype(absolute(RDF_DIR_LANG_STRING)),
		),
	];

	for (case, annotation) in cases {
		let mut interpreter = Interpreter::new(None)?;
		let statement = anywhere(about_s(Term::Literal(Literal {
			lexical_form: "Hei".to_owned(),
			annotation: Some(annotation),
		})));

		let error = expect_error(&mut interpreter, &statement)?;

		assert_eq!(
			error.kind,
			ErrorKind::InvalidLiteral,
			"{case} should be refused as an invalid literal"
		);
	}

	Ok(())
}

/// A triple term in the object position stands for the triple at its position in the
/// quad's list, and a triple term inside that triple refers to an earlier entry. The
/// interpreter builds the nested terms RDF 1.2 describes.
#[test]
fn builds_nested_triple_terms_from_the_quads_list() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;
	let inner = Triple {
		subject: Node::Iri(absolute("https://example.org/alice")),
		predicate: absolute("https://example.org/age"),
		object: plain("42"),
	};
	let outer = Triple {
		subject: Node::Iri(absolute("https://example.org/bob")),
		predicate: absolute("https://example.org/says"),
		object: Term::TripleTerm(0),
	};

	let quad = expect_quad(
		&mut interpreter,
		Statement::Quad(Quad {
			subject: Node::Iri(absolute("https://example.org/claim")),
			predicate: absolute("http://www.w3.org/1999/02/22-rdf-syntax-ns#reifies"),
			object: Term::TripleTerm(1),
			graph_name: None,
			triple_terms: vec![inner, outer],
		}),
	)?;

	let oxrdf::Term::Triple(outer) = &quad.object else {
		return Err(format!("expected a triple term, but the object is {}", quad.object).into());
	};
	let oxrdf::Term::Triple(inner) = &outer.object else {
		return Err(format!("expected a nested triple term, but it is {}", outer.object).into());
	};
	assert_eq!(triple_object_literal(inner)?.value(), "42");

	Ok(())
}

/// A triple term must refer to an entry that exists, and an entry may refer only to entries
/// before it. Anything else would let a parser describe a cycle, which RDF cannot hold.
#[test]
fn refuses_a_triple_term_that_refers_forwards_or_nowhere() -> TestResult {
	let self_referring = Triple {
		subject: Node::Iri(absolute("https://example.org/s")),
		predicate: absolute("https://example.org/p"),
		object: Term::TripleTerm(0),
	};
	let cases = [
		(
			"an entry that does not exist",
			Term::TripleTerm(3),
			Vec::new(),
		),
		(
			"an entry that refers to itself",
			Term::TripleTerm(0),
			vec![self_referring],
		),
	];

	for (case, object, triple_terms) in cases {
		let mut interpreter = Interpreter::new(None)?;
		let statement = anywhere(Statement::Quad(Quad {
			subject: Node::Iri(absolute("https://example.org/s")),
			predicate: absolute("https://example.org/p"),
			object,
			graph_name: None,
			triple_terms,
		}));

		let error = expect_error(&mut interpreter, &statement)?;

		assert_eq!(
			error.kind,
			ErrorKind::InvalidTripleTerm,
			"{case} should be refused"
		);
	}

	Ok(())
}

/// TriG and N-Quads documents keep data apart in named graphs. A quad with a graph name
/// belongs to that graph, and a quad without one belongs to the default graph.
#[test]
fn places_quads_in_named_and_default_graphs() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;

	let named = expect_quad(
		&mut interpreter,
		Statement::Quad(Quad {
			subject: Node::Iri(absolute("https://example.org/s")),
			predicate: absolute("https://example.org/p"),
			object: plain("o"),
			graph_name: Some(Node::Iri(absolute("https://example.org/graph"))),
			triple_terms: Vec::new(),
		}),
	)?;
	let default = expect_quad(&mut interpreter, about_s(plain("o")))?;

	assert_eq!(
		named.graph_name,
		oxrdf::GraphName::NamedNode(oxrdf::NamedNode::new("https://example.org/graph")?)
	);
	assert_eq!(default.graph_name, oxrdf::GraphName::DefaultGraph);

	Ok(())
}

/// Declarations change what later statements mean but produce no data of their own. A
/// warning comes back to the host with the span the parser gave it, so a person can be
/// told what was dropped and where.
#[test]
fn reports_declarations_versions_and_warnings() -> TestResult {
	let mut interpreter = Interpreter::new(None)?;

	let declaration = interpreter.interpret(&anywhere(Statement::Prefix {
		prefix: "ex".to_owned(),
		namespace: absolute("https://example.org/"),
	}))?;
	let version = interpreter.interpret(&anywhere(Statement::Version("1.2".to_owned())))?;
	let warning = interpreter.interpret(&at(
		40,
		52,
		Statement::Warning("a key was dropped".to_owned()),
	))?;

	assert_eq!(declaration, Interpretation::Declaration);
	assert_eq!(version, Interpretation::Declaration);
	assert_eq!(interpreter.version(), Some("1.2"));
	assert_eq!(
		warning,
		Interpretation::Warning {
			message: "a key was dropped".to_owned(),
			span: Span { start: 40, end: 52 },
		}
	);

	Ok(())
}

/// The starting base must itself be a valid absolute IRI, or nothing relative could be
/// resolved against it.
#[test]
fn refuses_a_starting_base_that_is_not_an_absolute_iri() {
	assert!(Interpreter::new(Some("not a base")).is_err());
}
