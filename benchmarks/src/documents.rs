// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Generated documents to benchmark with.
//!
//! Each document describes a number of people, with the same ten triples about each one,
//! so its size grows in steps of [`TRIPLES_PER_PERSON`] and the benchmark can report
//! throughput in triples. Generating the documents, rather than shipping sample files,
//! lets the benchmark run at any size and keeps the repository free of large test data.
//!
//! The Turtle uses each construct that gives the host something to interpret: prefixed
//! names, relative references, the `a` keyword, unquoted numbers, property lists in square
//! brackets, labelled blank nodes and language tags. The N-Triples states the same triples
//! with every name written in full, for timing the kits' N-Triples examples.

use std::fmt::Write;

/// The base every generated Turtle document is written against. Relative references in
/// the documents resolve against it.
pub const BASE: &str = "https://example.org/";

/// The number of triples a document states about each person.
pub const TRIPLES_PER_PERSON: usize = 10;

/// The namespace of the people, which the Turtle abbreviates as `ex:`.
const PEOPLE: &str = "https://example.org/people/";

/// The Friend of a Friend (FOAF) vocabulary, which the Turtle abbreviates as `foaf:`.
const FOAF: &str = "http://xmlns.com/foaf/0.1/";

/// A Turtle document describing `people` people.
///
/// The ten triples about each person are: their type, written with `a`; their name, with a
/// language tag; their age, as an unquoted integer; their home page, as a relative
/// reference; the two people they know, as prefixed names; an account, as a property list
/// in square brackets, with the account's name inside it; and the place they are based
/// near, as a labelled blank node, with the place's name in a statement of its own.
#[must_use]
pub fn turtle(people: usize) -> String {
	let mut document =
		format!("@prefix ex: <{PEOPLE}> .\n@prefix foaf: <{FOAF}> .\n@base <{BASE}> .\n\n");
	for person in 1..=people {
		let (first, second) = acquaintances(person, people);
		// `write!` into a `String` returns a result only because the `Write` trait requires
		// one. Appending to a string cannot fail, so the result is discarded.
		let _ = write!(
			document,
			"ex:p{person} a foaf:Person ;\n\
			\tfoaf:name \"Person {person}\"@en ;\n\
			\tfoaf:age {age} ;\n\
			\tfoaf:homepage <people/p{person}/home> ;\n\
			\tfoaf:knows ex:p{first}, ex:p{second} ;\n\
			\tfoaf:account [ foaf:accountName \"p{person}\" ] ;\n\
			\tfoaf:based_near _:place{person} .\n\
			_:place{person} foaf:name \"Paikka {person}\"@fi .\n\n",
			age = age(person),
		);
	}
	document
}

/// An N-Triples document stating the same triples as [`turtle`] does for `people` people.
///
/// The account the Turtle writes in square brackets has no label there; here it needs one,
/// `_:account` followed by the person's number, because N-Triples has no square brackets.
#[must_use]
pub fn n_triples(people: usize) -> String {
	let mut document = String::new();
	for person in 1..=people {
		let (first, second) = acquaintances(person, people);
		let subject = format!("<{PEOPLE}p{person}>");
		// Appending to a string cannot fail, as in `turtle`.
		let _ = write!(
			document,
			"{subject} <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <{FOAF}Person> .\n\
			{subject} <{FOAF}name> \"Person {person}\"@en .\n\
			{subject} <{FOAF}age> \"{age}\"^^<http://www.w3.org/2001/XMLSchema#integer> .\n\
			{subject} <{FOAF}homepage> <{BASE}people/p{person}/home> .\n\
			{subject} <{FOAF}knows> <{PEOPLE}p{first}> .\n\
			{subject} <{FOAF}knows> <{PEOPLE}p{second}> .\n\
			{subject} <{FOAF}account> _:account{person} .\n\
			_:account{person} <{FOAF}accountName> \"p{person}\" .\n\
			{subject} <{FOAF}based_near> _:place{person} .\n\
			_:place{person} <{FOAF}name> \"Paikka {person}\"@fi .\n",
			age = age(person),
		);
	}
	document
}

/// The two people that `person` knows: the next two in order, counting on from the last
/// person to the first.
///
/// With three or more people, the two are always different people and neither is `person`,
/// so each person contributes two distinct triples. With fewer, the triples would repeat and
/// a document would state fewer triples than [`TRIPLES_PER_PERSON`] promises, so the
/// benchmark never generates fewer than three.
const fn acquaintances(person: usize, people: usize) -> (usize, usize) {
	(person % people + 1, (person + 1) % people + 1)
}

/// An age for `person`, from 18 to 97. The ages vary, so the grammar reads integers of more
/// than one length, as it would in real data.
const fn age(person: usize) -> usize {
	18 + person % 80
}
