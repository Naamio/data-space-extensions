// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Writing statements, as a document made them, back out as Turtle.
//!
//! The text keeps everything the host interprets on the typed path: prefixed names stay
//! prefixed, relative references stay relative, and each prefix and base declaration is
//! written where the document made it, so the host's parser resolves the text exactly as
//! the host's interpreter resolves the typed statements. Each triple goes on a line of its
//! own, with every term written in full, which is the simplest Turtle to parse.
//!
//! Blank node labels are rewritten, not copied. A label the document wrote, such as `_:b1`,
//! becomes `_:wb1`, and a node the grammar numbered, such as the third `[ ]`, becomes `_:g3`.
//! The two prefixes keep a written label from ever meeting a number, which would merge two
//! nodes. Labels belong to one document only, so renaming them consistently changes nothing
//! the document says.

use std::fmt::Write;

use naamio_space_kit_rdf::contract::terms::{
	Annotation, BaseDirection, BlankNode, IriReference, Literal, LocatedStatement, Node, Statement,
	Term, Triple,
};

/// The characters that cannot appear unescaped in a Turtle IRI reference, besides the
/// control characters and space, which are checked by code point.
const NOT_IN_IRI: &str = "<>\"{}|^`\\";

/// The characters a Turtle local name must escape with a backslash wherever they appear.
/// `-` and `.` are allowed unescaped in some positions and escaped in others; escaping
/// them everywhere is always valid, so they are included.
const ESCAPED_IN_LOCAL_NAME: &str = "~.-!$&'()*+,;=/?#@";

/// The statements, as Turtle text.
///
/// A version declaration and a warning carry nothing the dataset stores, so each is written
/// as a comment, which keeps the text readable by a Turtle 1.1 parser. A quad's graph name
/// is not written: Turtle has no syntax for one, and the Turtle grammar never reports one.
pub fn turtle(statements: &[LocatedStatement]) -> Vec<u8> {
	let mut text = String::new();
	for located in statements {
		// Appending to a `String` cannot fail; `write!` returns a result only because the
		// `Write` trait requires one, so it is discarded.
		let _ = match &located.statement {
			Statement::Quad(quad) => writeln!(
				text,
				"{} {} {} .",
				node(&quad.subject),
				iri_reference(&quad.predicate),
				term(&quad.object, &quad.triple_terms),
			),
			Statement::Prefix(declaration) => writeln!(
				text,
				"@prefix {}: {} .",
				declaration.prefix,
				iri_reference(&declaration.namespace),
			),
			Statement::Base(base) => writeln!(text, "@base {} .", iri_reference(base)),
			Statement::Version(version) => writeln!(text, "# version {version}"),
			Statement::Warning(warning) => writeln!(text, "# warning: {warning}"),
		};
	}
	text.into_bytes()
}

/// A subject, as Turtle.
fn node(node: &Node) -> String {
	match node {
		Node::Iri(reference) => iri_reference(reference),
		Node::BlankNode(blank_node) => self::blank_node(blank_node),
	}
}

/// An object, as Turtle. A triple term is looked up in `triple_terms` by its index and
/// written in RDF 1.2's `<<( … )>>` form; its own terms may refer to earlier entries.
fn term(term: &Term, triple_terms: &[Triple]) -> String {
	match term {
		Term::Iri(reference) => iri_reference(reference),
		Term::BlankNode(blank_node) => self::blank_node(blank_node),
		Term::Literal(literal) => self::literal(literal),
		Term::TripleTerm(index) => {
			let triple = usize::try_from(*index)
				.ok()
				.and_then(|index| triple_terms.get(index));
			triple.map_or_else(
				// An index outside the list is the grammar's mistake. Writing a term the
				// host's parser will refuse makes the text path fail loudly, as the typed
				// path does when the host meets the same index.
				|| "<<( missing triple term )>>".to_owned(),
				|triple| {
					format!(
						"<<( {} {} {} )>>",
						node(&triple.subject),
						iri_reference(&triple.predicate),
						self::term(&triple.object, triple_terms),
					)
				},
			)
		}
	}
}

/// An IRI reference, as written: absolute and relative references in angle brackets, with
/// the characters Turtle forbids there escaped, and prefixed names as a prefix and a local
/// name.
fn iri_reference(reference: &IriReference) -> String {
	match reference {
		IriReference::Absolute(iri) | IriReference::Relative(iri) => {
			let mut written = String::with_capacity(iri.len() + 2);
			written.push('<');
			for character in iri.chars() {
				if character <= ' ' || NOT_IN_IRI.contains(character) {
					let _ = write!(written, "\\u{:04X}", u32::from(character));
				} else {
					written.push(character);
				}
			}
			written.push('>');
			written
		}
		IriReference::Prefixed(name) => {
			let mut written = format!("{}:", name.prefix);
			for character in name.local.chars() {
				if ESCAPED_IN_LOCAL_NAME.contains(character) {
					written.push('\\');
				}
				written.push(character);
			}
			written
		}
	}
}

/// A blank node, under the label scheme described in the module documentation.
fn blank_node(blank_node: &BlankNode) -> String {
	match blank_node {
		BlankNode::Written(label) => format!("_:w{label}"),
		BlankNode::Generated(number) => format!("_:g{number}"),
	}
}

/// A literal, in double quotes with the characters a quoted string cannot hold escaped,
/// followed by its datatype or its language tag and base direction.
fn literal(literal: &Literal) -> String {
	let mut written = String::with_capacity(literal.lexical_form.len() + 2);
	written.push('"');
	for character in literal.lexical_form.chars() {
		match character {
			'"' => written.push_str("\\\""),
			'\\' => written.push_str("\\\\"),
			'\n' => written.push_str("\\n"),
			'\r' => written.push_str("\\r"),
			character => written.push(character),
		}
	}
	written.push('"');
	match &literal.annotation {
		None => {}
		Some(Annotation::Datatype(datatype)) => {
			written.push_str("^^");
			written.push_str(&iri_reference(datatype));
		}
		Some(Annotation::Language(language)) => {
			written.push('@');
			written.push_str(&language.tag);
			match language.direction {
				None => {}
				Some(BaseDirection::Ltr) => written.push_str("--ltr"),
				Some(BaseDirection::Rtl) => written.push_str("--rtl"),
			}
		}
	}
	written
}
