// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests for parsing Turtle through the first-party Turtle component.
//!
//! These tests run the real `turtle-parser` component, from the `components/turtle-parser`
//! submodule, the way a Solid server would: a profile document arrives with the address it
//! was retrieved from, the component reads its syntax, and the host turns what the
//! component wrote into Resource Description Framework (RDF) quads. They establish that the
//! division of labour holds end to end: the component reads Turtle, and the host resolves
//! Internationalized Resource Identifiers (IRIs), expands prefixes, gives blank nodes their
//! identity, and reports problems with lines and columns.

mod support;

use host::parsers::{ParseFailure, ParseOptions, ParserComponent};
use linked_data::dataset::Dataset;
use linked_data::interpretation::ErrorKind;
use linked_data::oxrdf::{self, GraphName, Literal, NamedNode, Quad};
use linked_data::positions::Position;
use support::TestError;

/// The result of a test below.
type TestResult = Result<(), TestError>;

/// The address the profile document is taken to have been retrieved from. Its relative
/// IRIs resolve against it.
const PROFILE_ADDRESS: &str = "https://pod.example/profile/card";

/// A small Solid profile document, written as people write Turtle: with prefixes, a
/// relative IRI for the person, `a`, a property list and a collection.
const PROFILE: &str = r#"@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix schema: <https://schema.org/> .

<#me> a foaf:Person ;
	foaf:name "Alice"@en ;
	foaf:knows [ foaf:name "Bob" ] ;
	schema:knowsLanguage ( "fi" "en" ) .
"#;

/// Loads the Turtle component, built from the submodule, into a new extension host.
fn turtle() -> Result<ParserComponent, TestError> {
	support::load_parser("components/turtle-parser", "turtle_parser.wasm")
}

/// Options for a Turtle document retrieved from `base`.
fn retrieved_from(base: &str) -> ParseOptions {
	ParseOptions {
		base: Some(base.to_owned()),
		media_type: Some("text/turtle".to_owned()),
	}
}

/// A Solid server routes `text/turtle` documents by the media type the component reports,
/// and needs to know that Turtle cannot hold named graphs before it offers the component a
/// dataset to write out later.
#[tokio::test]
async fn describes_itself_as_turtle() -> TestResult {
	let description = turtle()?.describe().await?;

	assert_eq!(description.name, "Turtle");
	assert_eq!(
		description.media_types.first().map(String::as_str),
		Some("text/turtle")
	);
	assert!(!description.features.datasets);
	assert!(description.features.triple_terms);

	Ok(())
}

/// The profile's relative `<#me>` resolves against the address the document came from,
/// and its prefixed names expand, so the dataset holds the IRIs a reader of the profile
/// expects. Neither step happens in the component.
#[tokio::test]
async fn resolves_and_expands_a_profile_into_full_iris() -> TestResult {
	let mut dataset = Dataset::new();

	turtle()?
		.parse(
			PROFILE.as_bytes(),
			&retrieved_from(PROFILE_ADDRESS),
			&mut dataset,
		)
		.await?;

	let name = Quad::new(
		NamedNode::new("https://pod.example/profile/card#me")?,
		NamedNode::new("http://xmlns.com/foaf/0.1/name")?,
		Literal::new_language_tagged_literal("Alice", "en")?,
		GraphName::DefaultGraph,
	);
	assert!(
		dataset.contains(&name),
		"the dataset should state the profile's name"
	);

	Ok(())
}

/// Every triple the profile states arrives: `a`, the name, the property list's two quads,
/// and a collection of two items, which RDF writes as four list quads plus the quad that
/// points at the list.
#[tokio::test]
async fn stores_every_triple_the_profile_states() -> TestResult {
	let mut dataset = Dataset::new();

	let report = turtle()?
		.parse(
			PROFILE.as_bytes(),
			&retrieved_from(PROFILE_ADDRESS),
			&mut dataset,
		)
		.await?;

	assert_eq!(report.quads_added, 9);
	assert_eq!(dataset.len(), 9);

	Ok(())
}

/// An RDF 1.2 triple term reaches the dataset as a triple term, with the IRIs inside it
/// resolved like any others.
#[tokio::test]
async fn stores_triple_terms() -> TestResult {
	let mut dataset = Dataset::new();
	let document =
		"<claim> <http://www.w3.org/1999/02/22-rdf-syntax-ns#reifies> <<( <#me> <age> 42 )>> .";

	turtle()?
		.parse(
			document.as_bytes(),
			&retrieved_from(PROFILE_ADDRESS),
			&mut dataset,
		)
		.await?;

	let triple_terms: Vec<oxrdf::Triple> = dataset
		.quads()
		.filter_map(|quad| match quad.object {
			oxrdf::TermRef::Triple(triple) => Some(triple.clone()),
			_ => None,
		})
		.collect();
	let [triple] = triple_terms.as_slice() else {
		return Err(format!("expected one triple term, but found {triple_terms:?}").into());
	};
	assert_eq!(
		triple.subject,
		oxrdf::NamedOrBlankNode::NamedNode(NamedNode::new("https://pod.example/profile/card#me")?)
	);

	Ok(())
}

/// A document with a mistake on its third line is refused with the line and column of the
/// mistake, and the quads before it stay in the dataset for the host to keep or discard.
#[tokio::test]
async fn reports_a_syntax_error_with_its_line_and_column() -> TestResult {
	let mut dataset = Dataset::new();
	let document = "<a> <b> <c> .\n<d> <e> <f> .\n<g> <h> \"never closed .\n";

	let outcome = turtle()?
		.parse(
			document.as_bytes(),
			&retrieved_from(PROFILE_ADDRESS),
			&mut dataset,
		)
		.await;

	let Err(ParseFailure::Syntax { position, .. }) = outcome else {
		return Err(format!("expected a syntax error, but the outcome was {outcome:?}").into());
	};
	assert_eq!(position, Some(Position { line: 3, column: 9 }));
	assert_eq!(dataset.len(), 2);

	Ok(())
}

/// A prefix the document never declared is valid Turtle syntax but has no meaning. The
/// component passes it on, and the host, which owns prefixes, refuses it with its position.
#[tokio::test]
async fn reports_an_undeclared_prefix_from_the_host() -> TestResult {
	let mut dataset = Dataset::new();
	let document = "<a> <b> <c> .\nex:d <e> <f> .\n";

	let outcome = turtle()?
		.parse(
			document.as_bytes(),
			&retrieved_from(PROFILE_ADDRESS),
			&mut dataset,
		)
		.await;

	let Err(ParseFailure::Interpretation { error, position }) = outcome else {
		return Err(format!(
			"expected the host to refuse a statement, but the outcome was {outcome:?}"
		)
		.into());
	};
	assert_eq!(error.kind, ErrorKind::UndefinedPrefix);
	assert_eq!(position.map(|position| position.line), Some(2));

	Ok(())
}
