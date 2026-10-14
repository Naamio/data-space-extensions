// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests establishing that the benchmark compares like with like.
//!
//! The benchmark times three ways of turning a Turtle document into a dataset: the typed
//! path, where a component streams typed statements that the host interprets; the text
//! path, where the same grammar writes those statements as Turtle that the host parses; and
//! the native path, where the host parses the document itself. A timing means something
//! only if every path does the same work, so these tests check that each one stores the
//! same quads from the same document, and that the documents the benchmark generates are
//! the size it says they are.

// The host's test support builds components and loads them into a host. The benchmark
// uses the same code, so its components are built the same way as the host tests' are.
#[path = "../../host/tests/support/mod.rs"]
mod support;

use std::collections::BTreeMap;

use benchmarks::documents::{self, BASE, TRIPLES_PER_PERSON};
use benchmarks::native;
use benchmarks::text::TextParserComponent;
use host::parsers::ParseOptions;
use linked_data::dataset::Dataset;
use linked_data::oxrdf::{NamedOrBlankNodeRef, TermRef};
use support::TestError;

/// The result each test returns, so that a component failing to build or load is reported
/// with its cause instead of as a panic.
type TestResult = Result<(), TestError>;

/// The number of people the documents below describe: enough for every construct the
/// generator writes to occur many times, few enough to keep the tests quick.
const PEOPLE: usize = 50;

/// The package of the benchmark's component, and the file Cargo writes for it.
///
/// The one component serves both component paths. It exports the contract's `parser`
/// interface, which the host loads for the typed path, and the benchmark's `text-parser`
/// interface for the text path. Both run the same grammar in the same binary.
const COMPONENT: (&str, &str) = ("benchmarks/turtle-text-parser", "turtle_text_parser.wasm");

/// The options the typed path parses with: the base the generated documents are written
/// against, and Turtle's media type.
fn turtle_options() -> ParseOptions {
	ParseOptions {
		base: Some(BASE.to_owned()),
		media_type: Some("text/turtle".to_owned()),
	}
}

/// Every quad in `dataset`, written out with each blank node replaced by `_:`, and counted.
///
/// Each path gives blank nodes identifiers of its own, so two datasets holding the same
/// facts differ in those identifiers. Comparing the quads with blank nodes erased, together
/// with how many times each occurs, compares what the datasets say about named terms. It
/// would not notice a blank node joined to the wrong quads, which only a full isomorphism
/// check would. The `linked-data` crate's interpretation tests cover blank node identity,
/// so here the comparison only has to show that each path stores the same number of each
/// kind of quad.
fn without_blank_node_identity(dataset: &Dataset) -> BTreeMap<String, usize> {
	let mut quads = BTreeMap::new();
	for quad in dataset.quads() {
		let subject = match quad.subject {
			NamedOrBlankNodeRef::BlankNode(_) => "_:".to_owned(),
			NamedOrBlankNodeRef::NamedNode(node) => node.to_string(),
		};
		let object = match quad.object {
			TermRef::BlankNode(_) => "_:".to_owned(),
			object => object.to_string(),
		};
		*quads
			.entry(format!("{subject} {} {object}", quad.predicate))
			.or_insert(0) += 1;
	}
	quads
}

/// The benchmark reports throughput in triples, so each generated document must hold
/// exactly the number of triples the generator says it does, none of them repeated.
#[test]
fn generated_documents_hold_the_triples_they_promise() -> TestResult {
	let mut dataset = Dataset::new();

	let added = native::read_turtle(documents::turtle(PEOPLE).as_bytes(), BASE, &mut dataset)?;

	assert_eq!(added, PEOPLE * TRIPLES_PER_PERSON);

	Ok(())
}

/// The typed and text paths run the same grammar in the same component; only the form the
/// statements cross the boundary in differs. The datasets they produce must match, or the
/// text path would be skipping work the typed path does, or the reverse.
#[tokio::test]
async fn typed_and_text_paths_store_the_same_quads() -> TestResult {
	let document = documents::turtle(PEOPLE);
	let (package, artefact) = COMPONENT;
	let mut typed = Dataset::new();
	let mut text = Dataset::new();

	support::load_parser(package, artefact)?
		.parse(document.as_bytes(), &turtle_options(), &mut typed)
		.await?;
	TextParserComponent::load(&std::fs::read(support::build_rust_component(
		package, artefact,
	)?)?)?
	.parse(document.as_bytes(), BASE, &mut text)
	.await?;

	assert_eq!(typed.len(), PEOPLE * TRIPLES_PER_PERSON);
	assert_eq!(
		without_blank_node_identity(&typed),
		without_blank_node_identity(&text)
	);

	Ok(())
}

/// The native path is the floor the other two are measured against, so it must store the
/// same quads as the typed path.
#[tokio::test]
async fn native_path_stores_the_same_quads_as_the_typed_path() -> TestResult {
	let document = documents::turtle(PEOPLE);
	let (package, artefact) = COMPONENT;
	let mut typed = Dataset::new();
	let mut native = Dataset::new();

	support::load_parser(package, artefact)?
		.parse(document.as_bytes(), &turtle_options(), &mut typed)
		.await?;
	native::read_turtle(document.as_bytes(), BASE, &mut native)?;

	assert_eq!(
		without_blank_node_identity(&typed),
		without_blank_node_identity(&native)
	);

	Ok(())
}

/// The kits' N-Triples examples are timed against native N-Triples parsing, so the
/// generated N-Triples document must state the same quads as the Turtle one.
#[test]
fn generated_n_triples_states_what_the_turtle_states() -> TestResult {
	let mut from_turtle = Dataset::new();
	let mut from_n_triples = Dataset::new();

	native::read_turtle(documents::turtle(PEOPLE).as_bytes(), BASE, &mut from_turtle)?;
	native::read_n_triples(documents::n_triples(PEOPLE).as_bytes(), &mut from_n_triples)?;

	assert_eq!(
		without_blank_node_identity(&from_turtle),
		without_blank_node_identity(&from_n_triples)
	);

	Ok(())
}

/// A document the grammar refuses must fail on the text path too, rather than giving the
/// host a partial text that parses cleanly.
#[tokio::test]
async fn text_path_reports_the_errors_the_grammar_finds() -> TestResult {
	let (package, artefact) = COMPONENT;
	let component = TextParserComponent::load(&std::fs::read(support::build_rust_component(
		package, artefact,
	)?)?)?;
	let mut dataset = Dataset::new();

	let outcome = component
		.parse(b"<#me> <#name> \"never closed .\n", BASE, &mut dataset)
		.await;

	assert!(outcome.is_err(), "the text path accepted a broken document");

	Ok(())
}
