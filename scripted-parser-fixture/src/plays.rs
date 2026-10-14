// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! The scenarios the fixture can play.
//!
//! Each scenario is a fixed sequence of statements and an outcome, chosen to exercise one
//! behaviour of the host. A statement written "on line n" carries the span of line n of the
//! document the host sent, counting lines from one, so the host's tests know which line
//! every error and warning should point at.
//!
//! Most statements name their subjects and predicates by absolute Internationalized
//! Resource Identifier (IRI), which the host uses as written. Scenarios that test the
//! host's own work use prefixed names or relative references instead, which the host has
//! to expand or resolve.

use naamio_space_kit_rdf::prelude::*;

/// The namespace the scenarios declare as the prefix `ex:`, and the namespace of every
/// subject they name.
const EXAMPLE: &str = "https://example.org/";

/// The Friend of a Friend (FOAF) property that gives a person's name, used as the predicate
/// of most quads so that each quad differs only in what the test is about.
const FOAF_NAME: &str = "http://xmlns.com/foaf/0.1/name";

/// The FOAF property that links two people, used where a quad needs an IRI as its object.
const FOAF_KNOWS: &str = "http://xmlns.com/foaf/0.1/knows";

/// The number of quads the `many` scenario writes. One write to a stream carries a limited
/// number of items, so this many forces the host to read the stream over many writes.
const MANY: u64 = 5_000;

/// The lines of the document the host sent, used to give each statement a span.
#[derive(Debug)]
struct Document<'lines> {
	/// The span of each line, in order.
	lines: &'lines [Span],
}

impl Document<'_> {
	/// The span of line `line`, counting from one. A line beyond the end of the document
	/// takes the span of its last line, so a short document still gives every statement a
	/// span inside it.
	fn line(&self, line: usize) -> Span {
		self.lines
			.get(line.saturating_sub(1))
			.or_else(|| self.lines.last())
			.copied()
			.unwrap_or(Span { start: 0, end: 0 })
	}

	/// `statement`, placed on line `line` of the document.
	fn on_line(&self, line: usize, statement: Statement) -> LocatedStatement {
		located(self.line(line), statement)
	}
}

/// The scenario named by `media_type`, which has the form `scenario/<name>`, with spans
/// taken from `lines`.
///
/// A media type that names no scenario plays one that writes nothing and fails, so a typing
/// mistake in a test shows up as a failure rather than as an empty success.
pub fn play(media_type: Option<&str>, lines: &[Span]) -> Reading {
	let document = Document { lines };
	match media_type.and_then(|media_type| media_type.strip_prefix("scenario/")) {
		Some("people") => succeeds(people(&document)),
		Some("repeated") => succeeds(repeated(&document)),
		Some("relative") => succeeds(relative_references(&document)),
		Some("syntax-error") => syntax_error_on_line_three(&document),
		Some("undefined-prefix") => succeeds(undefined_prefix(&document)),
		Some("many") => succeeds(many(&document)),
		Some("warning") => succeeds(warning(&document)),
		Some("unsupported") => Reading {
			statements: Vec::new(),
			outcome: Err(ParseError::Unsupported("collections".to_owned())),
		},
		_ => Reading {
			statements: Vec::new(),
			outcome: Err(ParseError::Other(Some(format!(
				"no scenario is named by the media type {media_type:?}"
			)))),
		},
	}
}

/// Exercises the ordinary path: a declaration of `ex:` on line one, then three quads on
/// lines two to four whose names the host must expand through it.
fn people(document: &Document<'_>) -> Vec<LocatedStatement> {
	vec![
		document.on_line(
			1,
			Statement::Prefix(PrefixDeclaration {
				prefix: "ex".to_owned(),
				namespace: absolute(EXAMPLE),
			}),
		),
		document.on_line(2, named(example("alice"), "Alice")),
		document.on_line(
			3,
			quad(example("alice"), absolute(FOAF_KNOWS), example("bob")),
		),
		document.on_line(4, named(example("bob"), "Bob")),
	]
}

/// Exercises counting: the `people` statements, followed on line five by a repeat of the
/// quad on line four, which the parser has read but the dataset already holds.
fn repeated(document: &Document<'_>) -> Vec<LocatedStatement> {
	let mut statements = people(document);
	statements.push(document.on_line(5, named(example("bob"), "Bob")));
	statements
}

/// Exercises resolution: one quad on line one whose subject and object are the relative
/// references `alice` and `bob`, which only the base the host passes can resolve.
fn relative_references(document: &Document<'_>) -> Vec<LocatedStatement> {
	vec![document.on_line(
		1,
		quad(relative("alice"), absolute(FOAF_KNOWS), relative("bob")),
	)]
}

/// Exercises syntax errors: quads on lines one and two, then a syntax error on line three.
/// The two quads were delivered before the error, so the host keeps them.
fn syntax_error_on_line_three(document: &Document<'_>) -> Reading {
	Reading {
		statements: vec![
			document.on_line(1, named(example_iri("alice"), "Alice")),
			document.on_line(2, named(example_iri("bob"), "Bob")),
		],
		outcome: Err(syntax_error(
			"the scenario reports a syntax error on line three",
			document.line(3),
		)),
	}
}

/// Exercises the host's own refusals: a valid quad on line one, then on line two a quad
/// using `ex:` without declaring it. The host refuses line two and stops reading, so the
/// quad on line three never arrives.
fn undefined_prefix(document: &Document<'_>) -> Vec<LocatedStatement> {
	vec![
		document.on_line(1, named(example_iri("alice"), "Alice")),
		document.on_line(2, named(example("bob"), "Bob")),
		document.on_line(3, named(example_iri("carol"), "Carol")),
	]
}

/// Exercises long streams: [`MANY`] quads, all about the blank node `_:someone`, so the
/// host must read the stream over many writes and keep the label's node throughout.
fn many(document: &Document<'_>) -> Vec<LocatedStatement> {
	(0..MANY)
		.map(|index| {
			document.on_line(
				1,
				quad(
					labelled_blank_node("someone"),
					absolute(&format!("{EXAMPLE}p")),
					plain_literal(&index.to_string()),
				),
			)
		})
		.collect()
}

/// Exercises warnings: a quad on line one, then a warning on line two that the host must
/// report with its position.
fn warning(document: &Document<'_>) -> Vec<LocatedStatement> {
	vec![
		document.on_line(1, named(example_iri("alice"), "Alice")),
		document.on_line(2, Statement::Warning("a key was dropped".to_owned())),
	]
}

/// A reading that reports `statements` and then ends successfully.
const fn succeeds(statements: Vec<LocatedStatement>) -> Reading {
	Reading {
		statements,
		outcome: Ok(()),
	}
}

/// The statement that `subject` has the name `name`, using [`FOAF_NAME`]. Most scenarios
/// are built from these, varying only the subject.
fn named(subject: IriReference, name: &str) -> Statement {
	quad(subject, absolute(FOAF_NAME), plain_literal(name))
}

/// `local` in the example namespace, written as its absolute IRI, for statements the host
/// should accept without any expansion or resolution.
fn example_iri(local: &str) -> IriReference {
	absolute(&format!("{EXAMPLE}{local}"))
}

/// `local` in the example namespace, written as the prefixed name `ex:<local>`, which the
/// host must expand using the latest declaration of `ex:`.
fn example(local: &str) -> IriReference {
	prefixed("ex", local)
}
