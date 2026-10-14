// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Interpreting the statements a parser writes into Resource Description Framework (RDF)
//! quads.
//!
//! An [`Interpreter`] reads the statements of one document, in order. It keeps what the
//! document has declared so far, the base and the prefixes, and the blank nodes it has seen,
//! and turns each quad statement into an [`oxrdf::Quad`]. One interpreter serves one
//! document: blank node labels are scoped to their document, so a second document needs a
//! second interpreter.

use std::error::Error;
use std::fmt;

use crate::blank_nodes::BlankNodeScope;
use crate::namespaces::Namespaces;
use crate::terms;
use crate::written::{LocatedStatement, Span, Statement};

/// The meaning of one statement, once interpreted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interpretation {
	/// The statement was a quad, now fully resolved and checked.
	Quad(oxrdf::Quad),
	/// The statement was a declaration, such as a prefix, base or version declaration. It
	/// changes how later statements are read but adds no data of its own.
	Declaration,
	/// The parser accepted something a person should know about.
	Warning {
		/// The parser's description of it, in English.
		message: String,
		/// The bytes of the document it concerns.
		span: Span,
	},
}

/// The kinds of problem that stop a statement from being interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
	/// An Internationalized Resource Identifier (IRI) is not valid, as written or once
	/// resolved or expanded.
	InvalidIri,
	/// A relative reference appears where no base is in effect.
	MissingBase,
	/// A prefixed name uses a prefix the document has not declared.
	UndefinedPrefix,
	/// A literal breaks the rules of RDF, such as a malformed language tag.
	InvalidLiteral,
	/// A triple term refers to an entry that does not exist, or to one that is not earlier
	/// in its quad's list.
	InvalidTripleTerm,
}

/// A statement the interpreter could not accept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterpretationError {
	/// The kind of problem.
	pub kind: ErrorKind,
	/// A description of the fault, in English, naming the part of the statement at fault.
	pub message: String,
	/// The bytes of the document the statement came from, as the parser reported them.
	pub span: Span,
}

impl fmt::Display for InterpretationError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			formatter,
			"{} (bytes {} to {})",
			self.message, self.span.start, self.span.end
		)
	}
}

impl Error for InterpretationError {}

/// A problem found in part of a statement, before the interpreter knows which statement it
/// belongs to. The interpreter adds the statement's span and reports it as an
/// [`InterpretationError`].
#[derive(Debug)]
pub(crate) struct Problem {
	/// The kind of problem.
	pub kind: ErrorKind,
	/// A description of the fault, in English.
	pub message: String,
}

impl Problem {
	/// Places the problem in the statement it came from.
	fn located(self, span: Span) -> InterpretationError {
		InterpretationError {
			kind: self.kind,
			message: self.message,
			span,
		}
	}
}

/// A starting base that is not a valid absolute IRI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidBase {
	/// A description of the fault in the base, in English.
	pub message: String,
}

impl fmt::Display for InvalidBase {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			formatter,
			"the base is not a valid absolute IRI: {}",
			self.message
		)
	}
}

impl Error for InvalidBase {}

/// The interpreter for one document.
#[derive(Debug)]
pub struct Interpreter {
	/// The base and the prefixes in effect at the current point in the document.
	namespaces: Namespaces,
	/// The blank nodes the document has referred to so far.
	blank_nodes: BlankNodeScope,
	/// The version of the syntax the document declared, if it declared one.
	version: Option<String>,
}

impl Interpreter {
	/// Creates an interpreter for a new document.
	///
	/// `base` is the base IRI in effect before the document declares one of its own,
	/// usually the address the document was retrieved from. Without it, the document can
	/// use relative references only after declaring a base.
	///
	/// # Errors
	///
	/// Returns [`InvalidBase`] if `base` is not a valid absolute IRI.
	pub fn new(base: Option<&str>) -> Result<Self, InvalidBase> {
		Ok(Self {
			namespaces: Namespaces::new(base)?,
			blank_nodes: BlankNodeScope::default(),
			version: None,
		})
	}

	/// Interprets the next statement in the document.
	///
	/// Statements must arrive in the order the document makes them, because a prefix or
	/// base declaration changes how the statements after it are read.
	///
	/// # Errors
	///
	/// Returns an [`InterpretationError`] carrying the statement's span when the statement
	/// cannot be accepted. A refused statement adds no data and declares nothing, so the
	/// host may stop or carry on as it chooses. A blank node label it mentioned keeps the
	/// node it was given, which changes nothing a later statement can observe: a label means
	/// one node throughout its document.
	pub fn interpret(
		&mut self,
		located: &LocatedStatement,
	) -> Result<Interpretation, InterpretationError> {
		let located_error = |problem: Problem| problem.located(located.span);

		match &located.statement {
			Statement::Quad(quad) => terms::quad(quad, &self.namespaces, &mut self.blank_nodes)
				.map(Interpretation::Quad)
				.map_err(located_error),
			Statement::Prefix { prefix, namespace } => {
				self.namespaces
					.declare_prefix(prefix, namespace)
					.map_err(located_error)?;
				Ok(Interpretation::Declaration)
			}
			Statement::Base(reference) => {
				self.namespaces
					.declare_base(reference)
					.map_err(located_error)?;
				Ok(Interpretation::Declaration)
			}
			Statement::Version(version) => {
				self.version = Some(version.clone());
				Ok(Interpretation::Declaration)
			}
			Statement::Warning(message) => Ok(Interpretation::Warning {
				message: message.clone(),
				span: located.span,
			}),
		}
	}

	/// The version of the syntax the document declared, if it has declared one so far.
	#[must_use]
	pub fn version(&self) -> Option<&str> {
		self.version.as_deref()
	}
}
