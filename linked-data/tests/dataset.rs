// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Feature tests for the dataset the host stores interpreted quads in.
//!
//! The dataset is where Resource Description Framework (RDF) data lives once the
//! interpreter has checked it. These tests establish two things the host relies on: a
//! dataset is a set, so a repeated quad is held once, and a host can bound how many quads
//! one document may add.

use std::error::Error;

use linked_data::dataset::{Dataset, LimitReached};
use linked_data::oxrdf::{GraphName, Literal, NamedNode, Quad};

/// A boxed error type broad enough for anything a test below might fail on.
type TestResult = Result<(), Box<dyn Error>>;

/// Builds a quad in the default graph giving `https://example.org/alice` the name `name`.
/// Different names give different quads; the same name gives the same quad.
fn alice_named(name: &str) -> Result<Quad, Box<dyn Error>> {
	Ok(Quad::new(
		NamedNode::new("https://example.org/alice")?,
		NamedNode::new("http://xmlns.com/foaf/0.1/name")?,
		Literal::new_simple_literal(name),
		GraphName::DefaultGraph,
	))
}

/// Inserting a quad the dataset does not hold adds it, and the host learns that it was new.
#[test]
fn adds_a_new_quad() -> TestResult {
	let mut dataset = Dataset::new();

	let added = dataset.insert(&alice_named("Alice")?)?;

	assert!(added);
	assert_eq!(dataset.len(), 1);

	Ok(())
}

/// An RDF dataset is a set of quads. Inserting a quad it already holds changes nothing, and
/// the host learns that nothing was added.
#[test]
fn holds_a_repeated_quad_once() -> TestResult {
	let mut dataset = Dataset::new();

	dataset.insert(&alice_named("Alice")?)?;
	let added_again = dataset.insert(&alice_named("Alice")?)?;

	assert!(!added_again);
	assert_eq!(dataset.len(), 1);

	Ok(())
}

/// A host bounds what one document can add by giving the dataset a limit. A quad that would
/// take the dataset past it is refused and not stored.
#[test]
fn refuses_a_quad_beyond_the_limit() -> TestResult {
	let mut dataset = Dataset::with_quad_limit(1);

	dataset.insert(&alice_named("Alice")?)?;
	let outcome = dataset.insert(&alice_named("Alicia")?);

	assert_eq!(outcome, Err(LimitReached));
	assert_eq!(dataset.len(), 1);

	Ok(())
}

/// The limit counts quads the dataset holds. Inserting a quad it already has does not make
/// it larger, so a full dataset still accepts a repeat.
#[test]
fn accepts_a_repeated_quad_when_full() -> TestResult {
	let mut dataset = Dataset::with_quad_limit(1);

	dataset.insert(&alice_named("Alice")?)?;
	let outcome = dataset.insert(&alice_named("Alice")?);

	assert_eq!(outcome, Ok(false));

	Ok(())
}
