// SPDX-FileCopyrightText: 2026 Omnifi Foundation
// SPDX-License-Identifier: MPL-2.0

//! Running parser components.
//!
//! A [`ParserComponent`] is a component that exports the `parser` interface of the
//! `linked-data:rdf` contract, compiled and linked by the
//! [`ExtensionHost`](crate::runtime::ExtensionHost). Each parse runs in a new instance: the
//! host sends the document on a byte stream, takes the statements the component streams
//! back into a dataset as they arrive, and reports how the parse ended with lines and
//! columns a person can use.
//!
//! The types here follow the contract's, so that code calling the host never depends on
//! the generated bindings.

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use linked_data::dataset::Dataset;
use linked_data::interpretation::{InterpretationError, Interpreter, InvalidBase};
use linked_data::positions::{LineIndex, Position};
use linked_data::written::Span;
use tokio::sync::oneshot;
use wasmtime::component::StreamReader;
use wasmtime::{Engine, Store};

use crate::bindings::ParserComponentPre;
use crate::bindings::exports::linked_data::rdf::parser as contract;
use crate::bindings::linked_data::rdf::syntax;
use crate::ingestion::{Ingested, Ingestion, Stop};
use crate::instances::InstanceState;
use crate::runtime::ComponentError;
use crate::statements;
use crate::streams::{OutcomeConsumer, ParseOutcome, SharedIngestion, StatementConsumer};

/// The host's information about a document, given to a parser before it reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseOptions {
	/// The Internationalized Resource Identifier (IRI) the document was retrieved from, or
	/// the base the host chooses for it. Relative references in the document resolve
	/// against it until the document declares a base of its own.
	pub base: Option<String>,
	/// The media type the document arrived with, including any parameters.
	pub media_type: Option<String>,
}

/// The parts of the Resource Description Framework (RDF) data model a syntax can express
/// beyond triples in a single graph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyntaxFeatures {
	/// Named graphs, as in TriG and N-Quads.
	pub datasets: bool,
	/// Triple terms, as in RDF 1.2 syntaxes.
	pub triple_terms: bool,
	/// The base direction of literals, as in RDF 1.2 syntaxes.
	pub base_direction: bool,
}

/// A parser's description of the syntax it reads. A host routes each document to the
/// parser whose media types include the document's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDescription {
	/// The syntax's name, for people to read.
	pub name: String,
	/// The version of the syntax's specification the parser follows.
	pub version: String,
	/// The media types the syntax is registered under, the preferred one first.
	pub media_types: Vec<String>,
	/// The file extensions documents in the syntax usually have, without the dot.
	pub file_extensions: Vec<String>,
	/// The parts of RDF the syntax can express beyond triples in one graph.
	pub features: SyntaxFeatures,
}

/// Something a parser accepted but a person should know about, placed in the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
	/// The parser's description, in English.
	pub message: String,
	/// The bytes of the document it concerns.
	pub span: Span,
	/// The line and column those bytes start at, when the span lies in the document.
	pub position: Option<Position>,
}

/// The account of a successful parse: the quads read and added, the warnings, and the
/// version the document declared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseReport {
	/// Every quad the parser wrote, counting a quad each time it was written.
	pub quads_read: u64,
	/// The quads the dataset gained. Fewer than `quads_read` when the document repeats a
	/// quad, or states one the dataset already held.
	pub quads_added: u64,
	/// The warnings the parser wrote, in order.
	pub warnings: Vec<Warning>,
	/// The syntax version the document declared, if it declared one.
	pub version: Option<String>,
}

/// The reason a parse did not finish.
///
/// In every case, the quads taken before the failure stay in the dataset; the caller
/// decides whether to keep them.
#[derive(Debug)]
pub enum ParseFailure {
	/// The base in the options is not a valid absolute IRI. Nothing was parsed.
	InvalidBase(InvalidBase),
	/// The parser found the document invalid in its syntax.
	Syntax {
		/// The parser's description of the fault.
		message: String,
		/// The bytes at fault.
		span: Span,
		/// The line and column those bytes start at, when the span lies in the document.
		position: Option<Position>,
	},
	/// The document uses a feature of the syntax that the parser does not implement.
	Unsupported(String),
	/// The parser failed for another reason, with its description when it gave one.
	Other(Option<String>),
	/// The parser wrote a statement the host could not accept, such as one using a prefix
	/// it never declared. The host stopped reading at that statement.
	Interpretation {
		/// The interpreter's refusal, with the statement's span.
		error: InterpretationError,
		/// The line and column the statement starts at, when its span lies in the document.
		position: Option<Position>,
	},
	/// The dataset was full when the statement at `span` would have added a quad. The host
	/// stopped reading at that statement.
	LimitReached {
		/// The bytes of the statement that would have gone past the limit.
		span: Span,
		/// The line and column the statement starts at, when its span lies in the document.
		position: Option<Position>,
	},
	/// The component could not be instantiated or called, trapped, or ended without
	/// reporting how its parse ended.
	Component(ComponentError),
}

impl fmt::Display for ParseFailure {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidBase(error) => error.fmt(formatter),
			Self::Syntax {
				message, position, ..
			} => {
				write!(formatter, "syntax error{}: {message}", at(*position))
			}
			Self::Unsupported(feature) => {
				write!(formatter, "the parser does not implement {feature}")
			}
			Self::Other(Some(message)) => write!(formatter, "the parser failed: {message}"),
			Self::Other(None) => formatter.write_str("the parser failed without saying why"),
			Self::Interpretation { error, position } => {
				write!(formatter, "{}{}", error.message, at(*position))
			}
			Self::LimitReached { position, .. } => {
				write!(formatter, "the dataset reached its limit{}", at(*position))
			}
			Self::Component(error) => error.fmt(formatter),
		}
	}
}

impl Error for ParseFailure {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			Self::InvalidBase(error) => Some(error),
			Self::Interpretation { error, .. } => Some(error),
			Self::Component(error) => Some(error),
			_ => None,
		}
	}
}

/// Describes where in the document something is, for a message: " at line 3, column 1",
/// or nothing when the position is unknown.
fn at(position: Option<Position>) -> String {
	position.map_or_else(String::new, |position| {
		format!(" at line {}, column {}", position.line, position.column)
	})
}

/// A parser component, compiled and linked, ready to parse documents.
pub struct ParserComponent {
	/// The engine the component was compiled for, which every new instance runs on.
	engine: Engine,
	/// The component, already linked against the host's interfaces.
	prepared: ParserComponentPre<InstanceState>,
}

/// Prints the type name only; the compiled component holds nothing a reader of a debug
/// message could use.
impl fmt::Debug for ParserComponent {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("ParserComponent")
			.finish_non_exhaustive()
	}
}

impl ParserComponent {
	/// Wraps a component that [`ExtensionHost::load_parser`](crate::runtime::ExtensionHost::load_parser)
	/// has compiled and linked.
	pub(crate) const fn new(engine: Engine, prepared: ParserComponentPre<InstanceState>) -> Self {
		Self { engine, prepared }
	}

	/// Asks the component which syntax it reads.
	///
	/// # Errors
	///
	/// Returns a [`ComponentError`] if the component cannot be instantiated or traps.
	pub async fn describe(&self) -> Result<SyntaxDescription, ComponentError> {
		let mut store = Store::new(&self.engine, InstanceState::new());
		let instance = self.prepared.instantiate_async(&mut store).await?;
		let parser = instance.linked_data_rdf_parser();
		let description = store
			.run_concurrent(async |accessor| parser.call_describe(accessor).await)
			.await??;
		Ok(syntax_description(description))
	}

	/// Parses `document` and adds the quads it states to `dataset`.
	///
	/// The parse runs in a new instance of the component, so nothing the component holds
	/// in memory carries over from one document to the next. The dataset is moved into the
	/// parse while it runs and moved back before this method returns, whatever the outcome.
	///
	/// # Errors
	///
	/// Returns a [`ParseFailure`] saying why the parse did not finish, with a line and
	/// column wherever the failure points into the document.
	pub async fn parse(
		&self,
		document: &[u8],
		options: &ParseOptions,
		dataset: &mut Dataset,
	) -> Result<ParseReport, ParseFailure> {
		let interpreter =
			Interpreter::new(options.base.as_deref()).map_err(ParseFailure::InvalidBase)?;
		let shared: SharedIngestion = Arc::new(Mutex::new(Some(Ingestion::new(
			interpreter,
			std::mem::take(dataset),
		))));

		let outcome = self.run(document, options, &shared).await;

		// The lock is poisoned only if taking a statement panicked. Recovering it returns
		// the caller's dataset; at worst, the counts miss the statement that panicked.
		let ingestion = shared.lock().unwrap_or_else(PoisonError::into_inner).take();
		let Some(ingestion) = ingestion else {
			return Err(ParseFailure::Component(ComponentError::host(
				"the parse lost track of its dataset",
			)));
		};
		let mut ingested = ingestion.finish();
		*dataset = std::mem::take(&mut ingested.dataset);

		conclude(&LineIndex::new(document), outcome, ingested)
	}

	/// Runs the component over `document`, with every statement it writes going into
	/// `shared`, and returns the outcome the component reports once it has finished.
	///
	/// The call, the attaching of the consumers, and the wait for the outcome all happen
	/// inside one `run_concurrent`, because the component does its work in a task it spawns
	/// after `parse` returns, and that task runs only while the store's event loop does.
	async fn run(
		&self,
		document: &[u8],
		options: &ParseOptions,
		shared: &SharedIngestion,
	) -> Result<ParseOutcome, ComponentError> {
		let mut store = Store::new(&self.engine, InstanceState::new());
		let instance = self.prepared.instantiate_async(&mut store).await?;
		let parser = instance.linked_data_rdf_parser();

		let document = StreamReader::new(&mut store, document.to_vec())?;
		let contract_options = contract::ParseOptions {
			base: options.base.clone(),
			media_type: options.media_type.clone(),
		};
		let consumer = StatementConsumer::new(Arc::clone(shared));

		store
			.run_concurrent(async |accessor| -> Result<ParseOutcome, ComponentError> {
				let (statements, outcome) = parser
					.call_parse(accessor, document, contract_options)
					.await?;
				let (sender, receiver) = oneshot::channel();
				accessor.with(|mut access| -> wasmtime::Result<()> {
					statements.pipe(&mut access, consumer)?;
					outcome.pipe(&mut access, OutcomeConsumer::new(sender))
				})?;
				receiver.await.map_err(|_| {
					ComponentError::host(
						"the component ended without reporting how its parse ended",
					)
				})
			})
			.await?
	}
}

/// Decides how a parse ended, from what the component reported and what the ingestion
/// saw, and places every span in the document.
///
/// A stop the host made comes first: once the host stops reading, the component's own
/// outcome describes a parse the host had already abandoned.
fn conclude(
	index: &LineIndex<'_>,
	outcome: Result<ParseOutcome, ComponentError>,
	ingested: Ingested,
) -> Result<ParseReport, ParseFailure> {
	match ingested.stop {
		Some(Stop::Refused(error)) => {
			let position = index.position(error.span.start);
			return Err(ParseFailure::Interpretation { error, position });
		}
		Some(Stop::LimitReached(span)) => {
			return Err(ParseFailure::LimitReached {
				span,
				position: index.position(span.start),
			});
		}
		None => {}
	}

	match outcome.map_err(ParseFailure::Component)? {
		Ok(()) => Ok(ParseReport {
			quads_read: ingested.quads_read,
			quads_added: ingested.quads_added,
			warnings: ingested
				.warnings
				.into_iter()
				.map(|warning| Warning {
					position: index.position(warning.span.start),
					message: warning.message,
					span: warning.span,
				})
				.collect(),
			version: ingested.version,
		}),
		Err(contract::ParseError::Syntax(error)) => {
			let span = statements::span(error.span);
			Err(ParseFailure::Syntax {
				message: error.message,
				span,
				position: index.position(span.start),
			})
		}
		Err(contract::ParseError::Unsupported(feature)) => Err(ParseFailure::Unsupported(feature)),
		Err(contract::ParseError::Other(message)) => Err(ParseFailure::Other(message)),
	}
}

/// The host's form of a syntax description a component returned.
fn syntax_description(description: syntax::SyntaxDescription) -> SyntaxDescription {
	SyntaxDescription {
		name: description.name,
		version: description.version,
		media_types: description.media_types,
		file_extensions: description.file_extensions,
		features: SyntaxFeatures {
			datasets: description
				.features
				.contains(syntax::SyntaxFeatures::DATASETS),
			triple_terms: description
				.features
				.contains(syntax::SyntaxFeatures::TRIPLE_TERMS),
			base_direction: description
				.features
				.contains(syntax::SyntaxFeatures::BASE_DIRECTION),
		},
	}
}
