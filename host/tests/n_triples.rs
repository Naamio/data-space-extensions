// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests for the kits' N-Triples examples, run through the host.
//!
//! Each kit has an N-Triples example: the Rust kit's from the `kits/rust` submodule, and the
//! JavaScript kit's, written in TypeScript, from `kits/javascript`. Each example's own code
//! is a syntax description, an N-Triples grammar and one line exporting them; its kit
//! supplies everything else. These tests load both the way Naamio Space would load any
//! syntax extension, and establish that an extension written with either kit behaves as the
//! `linked-data:rdf` contract requires: it describes its syntax, its statements become
//! quads, and its errors arrive with positions. Every test runs against both examples, so a
//! difference between the kits shows up as a failure naming the kit.

mod support;

use host::parsers::{ParseFailure, ParseOptions, ParserComponent};
use linked_data::dataset::Dataset;
use linked_data::oxrdf::{GraphName, Literal, NamedNode, NamedOrBlankNodeRef, Quad};
use support::TestError;

/// The result of a test below.
type TestResult = Result<(), TestError>;

/// An N-Triples document with a triple of IRIs, a blank node on two lines, a literal with a
/// language tag and a literal with a datatype.
const DOCUMENT: &str = r#"<https://example.org/alice> <http://xmlns.com/foaf/0.1/knows> _:bob .
_:bob <http://xmlns.com/foaf/0.1/name> "Bob"@en .
<https://example.org/alice> <https://example.org/age> "42"^^<http://www.w3.org/2001/XMLSchema#integer> .
"#;

/// The N-Triples example from each kit, by the kit's name, each loaded into a new extension
/// host.
fn n_triples_examples() -> Result<[(&'static str, ParserComponent); 2], TestError> {
	Ok([
		(
			"Rust",
			support::load_parser("kits/rust/examples/n-triples", "n_triples_extension.wasm")?,
		),
		(
			"JavaScript",
			support::load_javascript_parser(
				"kits/javascript",
				"examples/n-triples",
				"n_triples_extension.wasm",
			)?,
		),
	])
}

/// Options for an N-Triples document. N-Triples has no relative Internationalized Resource
/// Identifiers (IRIs), so no base is given.
fn n_triples_options() -> ParseOptions {
	ParseOptions {
		base: None,
		media_type: Some("application/n-triples".to_owned()),
	}
}

/// A host routes `application/n-triples` documents to the extension by the media type it
/// reports, and must know that N-Triples holds no named graphs before offering it any.
#[tokio::test]
async fn describes_itself_as_n_triples() -> TestResult {
	for (kit, example) in n_triples_examples()? {
		let description = example.describe().await?;

		assert_eq!(description.name, "N-Triples", "{kit} kit");
		assert_eq!(
			description.media_types,
			vec!["application/n-triples".to_owned()],
			"{kit} kit"
		);
		assert_eq!(
			description.file_extensions,
			vec!["nt".to_owned()],
			"{kit} kit"
		);
		assert!(!description.features.datasets, "{kit} kit");
	}

	Ok(())
}

/// Every triple the document states becomes a quad. `_:bob` appears on two lines and is one
/// node, which the host decides from the label the extension passed on, and the typed
/// literal keeps its datatype.
#[tokio::test]
async fn stores_the_triples_a_document_states() -> TestResult {
	let age = Quad::new(
		NamedNode::new("https://example.org/alice")?,
		NamedNode::new("https://example.org/age")?,
		Literal::new_typed_literal(
			"42",
			NamedNode::new("http://www.w3.org/2001/XMLSchema#integer")?,
		),
		GraphName::DefaultGraph,
	);

	for (kit, example) in n_triples_examples()? {
		let mut dataset = Dataset::new();

		let report = example
			.parse(DOCUMENT.as_bytes(), &n_triples_options(), &mut dataset)
			.await?;

		assert_eq!(report.quads_added, 3, "{kit} kit");
		assert!(dataset.contains(&age), "{kit} kit");
		let about_bob = dataset
			.quads()
			.filter(|quad| matches!(quad.subject, NamedOrBlankNodeRef::BlankNode(_)))
			.count();
		assert_eq!(
			about_bob, 1,
			"{kit} kit: `_:bob` should be the subject of one quad"
		);
	}

	Ok(())
}

/// N-Triples does no resolution, so an IRI with dot segments must reach the dataset exactly
/// as written. The extension marks it absolute, and the host uses it verbatim instead of
/// resolving it into `http://a.example/c`.
#[tokio::test]
async fn keeps_absolute_iris_exactly_as_written() -> TestResult {
	let document = "<http://a.example/b/../c> <https://example.org/p> \"o\" .\n";

	for (kit, example) in n_triples_examples()? {
		let mut dataset = Dataset::new();

		example
			.parse(document.as_bytes(), &n_triples_options(), &mut dataset)
			.await?;

		let subjects: Vec<String> = dataset
			.quads()
			.map(|quad| quad.subject.to_string())
			.collect();
		assert_eq!(
			subjects,
			vec!["<http://a.example/b/../c>".to_owned()],
			"{kit} kit"
		);
	}

	Ok(())
}

/// A line without its full stop is refused by the extension. The host reports the line it
/// is on and keeps the triple from the line before, as the contract specifies.
#[tokio::test]
async fn reports_a_syntax_error_with_its_line() -> TestResult {
	let document = "<https://example.org/s> <https://example.org/p> \"one\" .\n<https://example.org/s> <https://example.org/p> \"two\"\n";

	for (kit, example) in n_triples_examples()? {
		let mut dataset = Dataset::new();

		let outcome = example
			.parse(document.as_bytes(), &n_triples_options(), &mut dataset)
			.await;

		let Err(ParseFailure::Syntax { position, .. }) = outcome else {
			return Err(format!(
				"{kit} kit: expected a syntax error, but the outcome was {outcome:?}"
			)
			.into());
		};
		assert_eq!(position.map(|position| position.line), Some(2), "{kit} kit");
		assert_eq!(dataset.len(), 1, "{kit} kit");
	}

	Ok(())
}
