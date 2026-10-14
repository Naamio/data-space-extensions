// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Taking a document's statements into a dataset, one at a time, as they arrive.
//!
//! An [`Ingestion`] lasts for one parse. It owns the document's interpreter and, for the
//! length of the parse, the dataset the quads go into. Each statement is interpreted and,
//! if it is a quad, stored. The first statement that cannot be taken stops the ingestion,
//! which tells the stream consumer to stop reading, and so tells the component to stop
//! parsing.

use linked_data::dataset::Dataset;
use linked_data::interpretation::{Interpretation, InterpretationError, Interpreter};
use linked_data::written::{LocatedStatement, Span};

/// The reason an ingestion stopped before the component finished.
#[derive(Debug)]
pub enum Stop {
	/// The interpreter refused a statement, such as one using an undeclared prefix.
	Refused(InterpretationError),
	/// The dataset was full when the statement at this span would have added a quad.
	LimitReached(Span),
}

/// A warning a component wrote, with the span it gave.
#[derive(Debug)]
pub struct WrittenWarning {
	/// The component's description of what it accepted but a person should know about.
	pub message: String,
	/// The bytes of the document the warning concerns.
	pub span: Span,
}

/// The state of one document being taken into a dataset.
#[derive(Debug)]
pub struct Ingestion {
	/// The interpreter for this document, holding its base, prefixes and blank nodes.
	interpreter: Interpreter,
	/// The dataset the quads go into, borrowed from the caller for the length of the parse.
	dataset: Dataset,
	/// Every quad the component wrote, counting repeats.
	quads_read: u64,
	/// The quads the dataset did not already hold.
	quads_added: u64,
	/// The warnings the component wrote, in order.
	warnings: Vec<WrittenWarning>,
	/// The reason the ingestion stopped early, once it has.
	stop: Option<Stop>,
}

/// The result of an ingestion, once the parse is over.
#[derive(Debug)]
pub struct Ingested {
	/// The dataset, with every quad taken before any stop, to be returned to the caller.
	pub dataset: Dataset,
	/// Every quad the component wrote, counting repeats.
	pub quads_read: u64,
	/// The quads the dataset did not already hold.
	pub quads_added: u64,
	/// The warnings the component wrote, in order.
	pub warnings: Vec<WrittenWarning>,
	/// The syntax version the document declared, if it declared one.
	pub version: Option<String>,
	/// The reason the ingestion stopped early, if it did.
	pub stop: Option<Stop>,
}

impl Ingestion {
	/// Starts taking a document into `dataset`, with `interpreter` set up for the document's
	/// base.
	pub const fn new(interpreter: Interpreter, dataset: Dataset) -> Self {
		Self {
			interpreter,
			dataset,
			quads_read: 0,
			quads_added: 0,
			warnings: Vec::new(),
			stop: None,
		}
	}

	/// Interprets one statement and stores any quad it makes. Returns whether the
	/// ingestion wants further statements: `false` once a statement has been refused or the
	/// dataset is full, and for every statement after that.
	pub fn take(&mut self, located: &LocatedStatement) -> bool {
		if self.stop.is_some() {
			return false;
		}
		match self.interpreter.interpret(located) {
			Ok(Interpretation::Quad(quad)) => {
				self.quads_read += 1;
				match self.dataset.insert(&quad) {
					Ok(added) => self.quads_added += u64::from(added),
					Err(_) => self.stop = Some(Stop::LimitReached(located.span)),
				}
			}
			Ok(Interpretation::Declaration) => {}
			Ok(Interpretation::Warning { message, span }) => {
				self.warnings.push(WrittenWarning { message, span });
			}
			Err(error) => self.stop = Some(Stop::Refused(error)),
		}
		self.stop.is_none()
	}

	/// Ends the ingestion and hands back the dataset and everything counted along the way.
	pub fn finish(self) -> Ingested {
		Ingested {
			version: self.interpreter.version().map(str::to_owned),
			dataset: self.dataset,
			quads_read: self.quads_read,
			quads_added: self.quads_added,
			warnings: self.warnings,
			stop: self.stop,
		}
	}
}
