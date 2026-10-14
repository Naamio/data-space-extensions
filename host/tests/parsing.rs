// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests for running a parser component.
//!
//! The host sends a document to a parser component as a byte stream, reads back the
//! statements the component streams in return, interprets them, and stores the resulting
//! quads in a dataset. These tests exercise that whole path over real WebAssembly System
//! Interface (WASI) 0.3 streams and futures.
//!
//! The component they run is the `scripted-parser-fixture` in this repository, not a real
//! parser, so that the host's behaviour is tested apart from any syntax. The fixture reads
//! the whole document, then plays the scenario named by the media type the host passes,
//! such as `scenario/people`. Each statement it writes carries the span of one line of the
//! document, so the tests can check the positions the host reports.

mod support;

use std::collections::HashSet;

use host::parsers::{ParseFailure, ParseOptions, ParserComponent};
use host::runtime::ExtensionHost;
use linked_data::dataset::Dataset;
use linked_data::interpretation::ErrorKind;
use linked_data::oxrdf;
use linked_data::positions::Position;
use linked_data::written::Span;
use support::TestError;

/// The result of a test below.
type TestResult = Result<(), TestError>;

/// A document of five lines. Its content does not matter to the fixture; only its lines
/// do, because the fixture gives each statement the span of one line.
const DOCUMENT: &str = "line one\nline two\nline three\nline four\nline five\n";

/// Loads the fixture component, built from this repository, into a new extension host.
fn fixture() -> Result<ParserComponent, TestError> {
	support::load_parser("scripted-parser-fixture", "scripted_parser_fixture.wasm")
}

/// Options that ask the fixture to play the scenario called `name`, with no base IRI.
fn scenario(name: &str) -> ParseOptions {
	ParseOptions {
		base: None,
		media_type: Some(format!("scenario/{name}")),
	}
}

/// The span the fixture gives a statement written for line `line` of [`DOCUMENT`],
/// counting lines from one. The span covers the line without its line feed.
fn line_span(line: usize) -> Span {
	let mut start = 0;
	for (index, text) in DOCUMENT.split('\n').enumerate() {
		let end = start + text.len();
		if index + 1 == line {
			return Span {
				start: u64::try_from(start).unwrap_or(u64::MAX),
				end: u64::try_from(end).unwrap_or(u64::MAX),
			};
		}
		start = end + 1;
	}
	Span {
		start: u64::MAX,
		end: u64::MAX,
	}
}

/// Whether the dataset holds a quad whose subject is the IRI `iri`.
fn has_subject(dataset: &Dataset, iri: &str) -> bool {
	dataset.quads().any(|quad| {
		matches!(quad.subject, oxrdf::NamedOrBlankNodeRef::NamedNode(node) if node.as_str() == iri)
	})
}

/// A host routes documents to parsers by media type, so it needs each parser to say which
/// syntax it reads before it is given a document.
#[tokio::test]
async fn reads_the_syntax_a_parser_describes() -> TestResult {
	let parser = fixture()?;

	let description = parser.describe().await?;

	assert_eq!(description.name, "Scripted parser fixture");
	assert!(
		description
			.media_types
			.iter()
			.any(|media_type| media_type == "application/x-scripted-parser-fixture")
	);
	assert!(description.features.datasets);

	Ok(())
}

/// The `people` scenario declares a prefix and writes three quads that use it. Every quad
/// reaches the dataset with its prefix expanded, which shows the statements crossed the
/// stream and passed through the interpreter.
#[tokio::test]
async fn stores_the_quads_a_parser_streams() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	let report = parser
		.parse(DOCUMENT.as_bytes(), &scenario("people"), &mut dataset)
		.await?;

	assert_eq!(dataset.len(), 3);
	assert!(has_subject(&dataset, "https://example.org/alice"));
	assert_eq!(report.quads_read, 3);
	assert_eq!(report.quads_added, 3);

	Ok(())
}

/// The `repeated` scenario writes one of its quads twice. The report separates the quads
/// the parser wrote from the quads the dataset gained, because a dataset holds a repeated
/// quad once.
#[tokio::test]
async fn reports_quads_read_and_quads_added_separately() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	let report = parser
		.parse(DOCUMENT.as_bytes(), &scenario("repeated"), &mut dataset)
		.await?;

	assert_eq!(report.quads_read, 4);
	assert_eq!(report.quads_added, 3);

	Ok(())
}

/// The `relative` scenario writes relative references. The base the host passes in the
/// options, usually the address the document was retrieved from, resolves them.
#[tokio::test]
async fn resolves_relative_references_against_the_base_it_passes() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();
	let options = ParseOptions {
		base: Some("https://pod.example/people/".to_owned()),
		media_type: Some("scenario/relative".to_owned()),
	};

	parser
		.parse(DOCUMENT.as_bytes(), &options, &mut dataset)
		.await?;

	assert!(has_subject(&dataset, "https://pod.example/people/alice"));

	Ok(())
}

/// The `syntax-error` scenario writes two quads, then reports a syntax error on line
/// three. The host turns the error's span into the line and column a person needs, and
/// keeps the quads delivered before the error, as the contract specifies.
#[tokio::test]
async fn reports_a_syntax_error_with_its_position() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	let outcome = parser
		.parse(DOCUMENT.as_bytes(), &scenario("syntax-error"), &mut dataset)
		.await;

	let Err(ParseFailure::Syntax { span, position, .. }) = outcome else {
		return Err(format!("expected a syntax error, but the outcome was {outcome:?}").into());
	};
	assert_eq!(span, line_span(3));
	assert_eq!(position, Some(Position { line: 3, column: 1 }));
	assert_eq!(dataset.len(), 2);

	Ok(())
}

/// The `undefined-prefix` scenario uses a prefix it never declared, on line two. The
/// interpreter refuses that statement, and the host reports the refusal with its position
/// and stops reading the stream.
#[tokio::test]
async fn reports_a_statement_the_interpreter_refuses() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	let outcome = parser
		.parse(
			DOCUMENT.as_bytes(),
			&scenario("undefined-prefix"),
			&mut dataset,
		)
		.await;

	let Err(ParseFailure::Interpretation { error, position }) = outcome else {
		return Err(
			format!("expected an interpretation error, but the outcome was {outcome:?}").into(),
		);
	};
	assert_eq!(error.kind, ErrorKind::UndefinedPrefix);
	assert_eq!(position, Some(Position { line: 2, column: 1 }));
	assert_eq!(dataset.len(), 1);

	Ok(())
}

/// The `many` scenario writes five thousand quads about one blank node, far more than fit
/// in one write to the stream. All of them arrive, and they share one node, because the
/// interpreter for a document keeps its blank node labels for the whole stream.
#[tokio::test]
async fn reads_a_long_stream_to_its_end() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	parser
		.parse(DOCUMENT.as_bytes(), &scenario("many"), &mut dataset)
		.await?;

	let subjects: HashSet<oxrdf::NamedOrBlankNode> = dataset
		.quads()
		.map(|quad| quad.subject.into_owned())
		.collect();
	assert_eq!(dataset.len(), 5_000);
	assert_eq!(subjects.len(), 1);

	Ok(())
}

/// A host bounds what one document can add. When the dataset reaches its limit, the parse
/// stops with the span of the statement that would have gone past it.
#[tokio::test]
async fn stops_when_the_dataset_reaches_its_limit() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::with_quad_limit(2);

	let outcome = parser
		.parse(DOCUMENT.as_bytes(), &scenario("people"), &mut dataset)
		.await;

	let Err(ParseFailure::LimitReached { span, .. }) = outcome else {
		return Err(
			format!("expected the limit to be reached, but the outcome was {outcome:?}").into(),
		);
	};
	assert_eq!(span, line_span(4));
	assert_eq!(dataset.len(), 2);

	Ok(())
}

/// The `warning` scenario writes a quad and a warning on line two. The warning reaches the
/// report with its position, so the host can tell a person what the parser dropped.
#[tokio::test]
async fn reports_warnings_with_their_positions() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	let report = parser
		.parse(DOCUMENT.as_bytes(), &scenario("warning"), &mut dataset)
		.await?;

	let [warning] = report.warnings.as_slice() else {
		return Err(format!(
			"expected one warning, but the report has {:?}",
			report.warnings
		)
		.into());
	};
	assert_eq!(warning.message, "a key was dropped");
	assert_eq!(warning.position, Some(Position { line: 2, column: 1 }));

	Ok(())
}

/// The `unsupported` scenario ends the parse because the document uses a feature the
/// parser does not implement. The host passes the parser's explanation on.
#[tokio::test]
async fn reports_a_feature_the_parser_does_not_implement() -> TestResult {
	let parser = fixture()?;
	let mut dataset = Dataset::new();

	let outcome = parser
		.parse(DOCUMENT.as_bytes(), &scenario("unsupported"), &mut dataset)
		.await;

	assert!(
		matches!(&outcome, Err(ParseFailure::Unsupported(feature)) if feature == "collections"),
		"the outcome was {outcome:?}"
	);

	Ok(())
}

/// Bytes that are not a WebAssembly component are refused when they are loaded, before any
/// document is involved.
#[test]
fn refuses_bytes_that_are_not_a_component() -> TestResult {
	let host = ExtensionHost::new()?;

	assert!(host.load_parser(b"not a component").is_err());

	Ok(())
}
